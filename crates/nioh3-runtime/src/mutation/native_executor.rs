//! The concrete product live-add executor: the native transport that owns one
//! insertion and the adapter that inspects, waits and recovers around it.
//!
//! Port of `live_add_native_transport.NativeLiveAddTransport` and
//! `live_add_adapter.LiveAddAdapter`. The split is the shipped one: the
//! transport owns the durable receipt and the dispatch shim, the adapter owns
//! the identity, the plan and the wait.
//!
//! Every rule the shipped code states is kept:
//!
//! * admission refuses a second operation while a receipt is unresolved, and a
//!   missing file is never treated as proof that nothing was submitted;
//! * the receipt is durable before any native work, and it is immutable
//!   ownership evidence across restarts;
//! * a redirect that was not acknowledged is `uncertain`: the allocation and
//!   the debug ownership are retained, and nothing replays;
//! * a preview may retry only the explicit, fully released, zero-redirect idle
//!   miss; an insertion never retries;
//! * `recover` verifies, it never dispatches.

use crate::error::RuntimeError;
use crate::mutation::count::{new_operation_id, read_bytes, read_json, sha256_hex};
use crate::mutation::descriptor::{assembly_descriptor, verify_assembly_preview};
use crate::mutation::evidence::verify_dispatch;
use crate::mutation::inventory::{
    capture_read_only, hex_decode, Inventory, InventoryLayout, InventoryProcess, NativeIndex,
    RECORD_SIZE,
};
use crate::mutation::live_add::{InstallationCandidate, LiveAddExecutor, LIVE_ADD_DISPLAY_VERSION};
use crate::mutation::native_abi::{
    build_dispatch_code, hex, InsertionArgs, LiveAddLayout, BUILDER_RESULT_OFFSET,
    DISPATCH_CANARIES, DISPATCH_DESCRIPTOR_OFFSET, DISPATCH_MARKER_OFFSET,
    DISPATCH_REMAINDER_OFFSET, DISPATCH_SLOT_OFFSET, DISPATCH_SOURCE_OFFSET,
    DISPATCH_STATUS_OFFSET, INSERTION_RESULT_OFFSET, REMOTE_CODE_SIZE,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The three modes the shipped transport accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchMode {
    Preview,
    Insert,
    Noop,
}

impl DispatchMode {
    pub const fn value(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Insert => "insert",
            Self::Noop => "noop",
        }
    }

    /// The receipt's own `mode` field.
    pub const fn receipt_mode(self) -> &'static str {
        match self {
            Self::Insert => "single_native_insertion",
            Self::Preview => "preview",
            Self::Noop => "noop",
        }
    }
}

fn rejected(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::LiveAddRejected {
        detail: detail.into(),
    }
}

fn dispatch_error(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::NativeDispatch {
        detail: detail.into(),
    }
}

/// `released`, `inactive` and zero breakpoints: the shipped settled predicate.
pub fn settled(value: &Value) -> bool {
    value.get("released").and_then(Value::as_bool) == Some(true)
        && value.get("active").and_then(Value::as_bool) == Some(false)
        && value.get("breakpoint_count").and_then(Value::as_i64) == Some(0)
}

/// `live_add_profile.live_add_profile((2, 0, 1, 0))` acceptance.
pub const REQUIRED_DISPLAY_VERSION: &str = "PC v2.01";

/// The transport [crate::mutation::live_add::LiveAddExecutor] drives.
pub trait LiveAddTransport {
    fn pid(&self) -> u32;

    fn profile_id(&self) -> &'static str;

    fn module_base(&mut self) -> Result<u64, RuntimeError>;

    /// `ProcessReader.creation_time()` for the planned lifetime.
    fn creation_time(&mut self) -> Result<String, RuntimeError>;

    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError>;

    /// `ping`: the endpoint identity plus whether any native work is owned.
    fn ping(&mut self) -> Result<Value, RuntimeError>;

    /// `preview` / `insert` / `noop`. Admission happens here.
    fn dispatch(&mut self, mode: DispatchMode, params: &Value) -> Result<Value, RuntimeError>;

    /// `status`: read the durable receipt.
    fn status(&mut self, operation_id: &str) -> Result<Value, RuntimeError>;

    /// `release`: read the receipt, and settle it by observation when it can.
    fn release(&mut self, operation_id: &str) -> Result<Value, RuntimeError>;

    /// `operation_known`: local, provable submission ownership only.
    fn operation_known(&mut self, operation_id: &str) -> bool;

    /// No receipt may still own the target.
    fn safe_to_shutdown(&mut self) -> bool;
}

/// A view that lets the shipped inventory capture read through one transport.
pub struct TransportInventoryView<'a, T: LiveAddTransport + ?Sized> {
    transport: &'a mut T,
    module_base: u64,
    creation_time: String,
}

impl<T: LiveAddTransport + ?Sized> InventoryProcess for TransportInventoryView<'_, T> {
    fn pid(&self) -> u32 {
        self.transport.pid()
    }

    fn module_base(&self) -> u64 {
        self.module_base
    }

    fn creation_time(&mut self) -> Result<String, RuntimeError> {
        Ok(self.creation_time.clone())
    }

    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
        self.transport.read(address, size)
    }
}

/// The adapter: identity, plan, wait, recovery and the shutdown fact.
pub struct NativeLiveAddExecutor<T: LiveAddTransport> {
    transport: T,
    layout: LiveAddLayout,
    display_version: String,
    pending: Option<String>,
    pending_pid: Option<u32>,
    pending_creation_time: Option<String>,
    budget: DispatchBudget,
}

impl<T: LiveAddTransport> NativeLiveAddExecutor<T> {
    pub fn new(transport: T, layout: LiveAddLayout, display_version: &str) -> Self {
        Self {
            transport,
            layout,
            display_version: display_version.to_string(),
            pending: None,
            pending_pid: None,
            pending_creation_time: None,
            budget: DispatchBudget::PRODUCT,
        }
    }

    /// Override the wait budget; tests use a zero-poll budget.
    pub fn with_budget(mut self, budget: DispatchBudget) -> Self {
        self.budget = budget;
        self
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn layout(&self) -> &LiveAddLayout {
        &self.layout
    }

    /// `LiveAddAdapter.identity`: a typed refusal before anything is opened.
    fn require_supported_version(&self) -> Result<(), RuntimeError> {
        if self.display_version != REQUIRED_DISPLAY_VERSION
            || self.transport.profile_id() != self.layout.profile_id
        {
            return Err(dispatch_error("Live addition requires accepted PC v2.01"));
        }
        Ok(())
    }

    /// `LiveAddAdapter._refresh_pending`.
    fn refresh_pending(&mut self) {
        let Some(pending) = self.pending.clone() else {
            return;
        };
        if let Ok(value) = self.transport.status(&pending) {
            if value.get("operation_id").and_then(Value::as_str) == Some(pending.as_str())
                && settled(&value)
            {
                self.clear_pending();
            }
        }
    }

    fn clear_pending(&mut self) {
        self.pending = None;
        self.pending_pid = None;
        self.pending_creation_time = None;
    }

    /// `LiveAddAdapter.wait`: poll the durable receipt until it settles.
    fn wait(&mut self, operation_id: &str) -> Result<Value, RuntimeError> {
        if let Some(pending) = &self.pending {
            if pending != operation_id {
                return Err(rejected("Another native operation still owns the adapter"));
            }
        }
        for _ in 0..self.budget.polls.max(1) {
            let value = self.transport.status(operation_id)?;
            if value.get("operation_id").and_then(Value::as_str) != Some(operation_id) {
                return Err(dispatch_error("Native receipt operation identity differs"));
            }
            if settled(&value) {
                self.clear_pending();
                return Ok(normalize(value));
            }
            let inactive = value.get("phase").and_then(Value::as_str) == Some("completed")
                || value.get("active").and_then(Value::as_bool) != Some(true);
            if inactive && value.get("redirect_count").and_then(Value::as_u64) == Some(0) {
                // A dispatch that never redirected is released, not uncertain.
                let released = self.transport.release(operation_id)?;
                if settled(&released) {
                    self.clear_pending();
                    return Ok(normalize(released));
                }
            }
            if inactive {
                return Err(dispatch_error(
                    "Native dispatch result is uncertain; allocation retained, do not retry",
                ));
            }
            (self.budget.pause)();
        }
        Err(dispatch_error(
            "Native dispatch did not settle; query its receipt before any further operation",
        ))
    }

    /// `LiveAddAdapter._submit`.
    fn submit(
        &mut self,
        mode: DispatchMode,
        operation_id: &str,
        owner_pid: u32,
        fields: &Map<String, Value>,
    ) -> Result<Value, RuntimeError> {
        self.refresh_pending();
        if self.pending.is_some() {
            return Err(rejected(
                "Another native operation still owns the adapter; recover it first",
            ));
        }
        let mut params = fields.clone();
        params.insert(
            "operation_id".to_string(),
            Value::String(operation_id.to_string()),
        );
        params.insert("pid".to_string(), json!(owner_pid));
        params.insert(
            "profile_id".to_string(),
            Value::String(self.layout.profile_id.to_string()),
        );
        self.pending = Some(operation_id.to_string());
        self.pending_pid = Some(owner_pid);
        self.pending_creation_time = fields
            .get("process_creation_time")
            .and_then(Value::as_str)
            .map(str::to_string);
        match self.transport.dispatch(mode, &Value::Object(params)) {
            Ok(value) => Ok(value),
            Err(error) => {
                if !self.transport.operation_known(operation_id) {
                    // A proven pre-dispatch refusal keeps no ownership.
                    self.clear_pending();
                }
                Err(error)
            }
        }
    }

    /// Every fact `LiveAddAdapter.inspect` resolves, as the stored plan.
    pub fn inspect_plan(&mut self) -> Result<(Value, Inventory, NativeIndex), RuntimeError> {
        self.require_supported_version()?;
        let pid = self.transport.pid();
        let endpoint = self.transport.ping()?;
        let busy = endpoint.get("busy").and_then(Value::as_bool) == Some(true);
        if endpoint.get("pid").and_then(Value::as_u64) != Some(u64::from(pid))
            || endpoint.get("profile_id").and_then(Value::as_str) != Some(self.layout.profile_id)
            || busy
        {
            return Err(dispatch_error(
                "Live-add executor is busy or attached to a different process/profile",
            ));
        }
        let module_base = self.transport.module_base()?;
        let creation_time = self.transport.creation_time()?;
        let layout = inventory_layout(&self.layout);
        let (inventory, index) = {
            let mut view = TransportInventoryView {
                transport: &mut self.transport,
                module_base,
                creation_time: creation_time.clone(),
            };
            capture_read_only(&mut view, &layout, LIVE_ADD_DISPLAY_VERSION)?
        };
        if inventory.pid != pid || index.pid != pid {
            return Err(dispatch_error("Process changed during inspection"));
        }
        if inventory.process_creation_time != creation_time
            || index.process_creation_time != creation_time
        {
            return Err(RuntimeError::ProcessInstanceChanged { pid });
        }
        if inventory.acquisition_order_counter == u32::MAX {
            return Err(rejected(
                "Acquisition-order counter cannot be advanced without overflow",
            ));
        }
        let dispatch = self
            .transport
            .read(module_base + self.layout.dispatch_rva, 7)?;
        if dispatch != self.layout.dispatch_signature {
            return Err(dispatch_error("Dispatch instructions differ"));
        }
        let manager = read_u64(
            &mut self.transport,
            module_base + self.layout.manager_pointer_rva,
        )?;
        let data = read_u64(&mut self.transport, manager)?;
        let container = self.transport.read(
            data + self.layout.container_offset,
            self.layout.capacity as usize * RECORD_SIZE,
        )?;
        if sha256_hex(&container) != inventory.container_sha256 {
            return Err(dispatch_error("Inventory changed during planning"));
        }
        let serial = read_u64(
            &mut self.transport,
            data + self.layout.serial_counter_offset,
        )?;
        if serial.to_string() != inventory.serial_counter
            || serial == 0
            || serial >= 0x7FFF_FFFF_FFFF_FFFE
        {
            return Err(rejected(
                "Serial changed or exceeds this executor ABI range",
            ));
        }
        let slot = (0..self.layout.capacity as usize)
            .find(|slot| {
                let start = slot * RECORD_SIZE;
                container[start] == 0 && container[start + 1] == 0
            })
            .ok_or_else(|| rejected("Scroll inventory is full"))?;
        let scheduler = read_u64(
            &mut self.transport,
            module_base + self.layout.scheduler_pointer_rva,
        )?;
        let insertion_code = self.transport.read(
            module_base + self.layout.insertion_rva,
            self.layout.insertion_size as usize,
        )?;
        let builder_code = self.transport.read(
            module_base + self.layout.builder_rva,
            self.layout.builder_size as usize,
        )?;
        let plan = json!({
            "pid": pid,
            "profile_id": self.layout.profile_id,
            "manager": manager,
            "data": data,
            "process_creation_time": creation_time,
            "serial": serial,
            "slot": slot,
            "scheduler_owner": scheduler,
            "function_address": module_base + self.layout.insertion_rva,
            "container_hex": hex(&container),
            "insertion_code_hex": hex(&insertion_code),
            "builder_code_hex": hex(&builder_code),
        });
        Ok((plan, inventory, index))
    }

    /// `LiveAddAdapter.require_process_instance`.
    pub fn require_instance(
        &mut self,
        pid: u32,
        process_creation_time: Option<&str>,
    ) -> Result<(), RuntimeError> {
        let expected = process_creation_time.ok_or_else(|| {
            rejected("PROCESS_INSTANCE_CHANGED: do not verify an old receipt against a new game")
        })?;
        let current = self.transport.creation_time()?;
        if self.transport.pid() != pid || current != expected {
            return Err(RuntimeError::ProcessInstanceChanged {
                pid: self.transport.pid(),
            });
        }
        Ok(())
    }
}

/// The plan-side facts one dispatch request carries.
fn plan_fields(plan: &Value, keys: &[&str]) -> Map<String, Value> {
    let mut fields = Map::new();
    for key in keys {
        if let Some(value) = plan.get(*key) {
            fields.insert((*key).to_string(), value.clone());
        }
    }
    fields
}

impl<T: LiveAddTransport> LiveAddExecutor for NativeLiveAddExecutor<T> {
    fn inspect(&mut self) -> Result<(Value, Inventory, NativeIndex), RuntimeError> {
        self.inspect_plan()
    }

    /// `LiveAddAdapter.preview`: only the explicit, fully released,
    /// zero-redirect idle miss retries, and every attempt keeps its receipt.
    fn preview(&mut self, plan: &Value, assembly_record: &[u8]) -> Result<Value, RuntimeError> {
        let expected_record = hex(assembly_record);
        let descriptor = hex(&assembly_descriptor(assembly_record, false)?);
        let pid = plan.get("pid").and_then(Value::as_u64).unwrap_or(0) as u32;
        let mut last = None;
        for attempt in 0..3 {
            let operation_id = new_operation_id()?;
            let mut fields = plan_fields(
                plan,
                &[
                    "profile_id",
                    "process_creation_time",
                    "source_save_path",
                    "candidate_id",
                    "parent_operation_id",
                    "builder_code_hex",
                ],
            );
            fields.insert("descriptor_hex".to_string(), json!(descriptor));
            fields.insert("expected_record_hex".to_string(), json!(expected_record));
            self.submit(DispatchMode::Preview, &operation_id, pid, &fields)?;
            let result = self.wait(&operation_id)?;
            let idle_miss = result.get("redirect_count").and_then(Value::as_u64) == Some(0)
                && settled(&result)
                && result
                    .get("breakpoints")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    == Some(0)
                && result.get("error").and_then(Value::as_str)
                    == Some("No accepted idle dispatch before timeout");
            if idle_miss && attempt < 2 {
                last = Some(result);
                continue;
            }
            verify_dispatch(&result)?;
            let source = hex_decode(
                result
                    .get("source_hex")
                    .and_then(Value::as_str)
                    .ok_or_else(|| rejected("Stored plan content changed"))?,
            )?;
            verify_assembly_preview(assembly_record, &source)?;
            return Ok(result);
        }
        last.ok_or_else(|| dispatch_error("Preview retry loop did not return"))
    }

    /// `LiveAddAdapter.insert`: one submission, no retry, ever.
    fn insert(&mut self, plan: &Value) -> Result<Value, RuntimeError> {
        let operation_id = plan
            .get("operation_id")
            .and_then(Value::as_str)
            .ok_or_else(|| rejected("Stored plan content changed"))?
            .to_string();
        let fields = plan_fields(
            plan,
            &[
                "profile_id",
                "manager",
                "data",
                "serial",
                "slot",
                "scheduler_owner",
                "function_address",
                "container_hex",
                "insertion_code_hex",
                "builder_code_hex",
                "descriptor_hex",
                "expected_record_hex",
                "process_creation_time",
                "source_save_path",
                "candidate_id",
                "parent_operation_id",
            ],
        );
        let pid = plan.get("pid").and_then(Value::as_u64).unwrap_or(0) as u32;
        self.submit(DispatchMode::Insert, &operation_id, pid, &fields)?;
        self.wait(&operation_id)
    }

    /// `LiveAddAdapter.readback`.
    fn readback(&mut self) -> Result<(Inventory, NativeIndex), RuntimeError> {
        let module_base = self.transport.module_base()?;
        let creation_time = self.transport.creation_time()?;
        let layout = inventory_layout(&self.layout);
        let (inventory, index) = {
            let mut view = TransportInventoryView {
                transport: &mut self.transport,
                module_base,
                creation_time: creation_time.clone(),
            };
            capture_read_only(&mut view, &layout, LIVE_ADD_DISPLAY_VERSION)?
        };
        if inventory.pid != index.pid
            || inventory.process_creation_time != index.process_creation_time
        {
            return Err(dispatch_error(
                "PROCESS_INSTANCE_CHANGED: readback snapshots span game lifetimes",
            ));
        }
        // The readback proves the same process instance the plan named.
        if inventory.pid == self.transport.pid() && inventory.process_creation_time != creation_time
        {
            return Err(RuntimeError::ProcessInstanceChanged { pid: inventory.pid });
        }
        Ok((inventory, index))
    }

    /// `LiveAddAdapter.recover`: read the receipt, verify, never dispatch.
    fn recover(
        &mut self,
        operation_id: &str,
        pid: u32,
        process_creation_time: Option<&str>,
    ) -> Result<Value, RuntimeError> {
        self.refresh_pending();
        if let Some(pending) = &self.pending {
            if pending != operation_id {
                return Err(rejected("Another native operation still owns the adapter"));
            }
        }
        self.require_instance(pid, process_creation_time)?;
        self.pending_pid = Some(pid);
        self.pending_creation_time = process_creation_time.map(str::to_string);
        let receipt = self.transport.status(operation_id)?;
        if settled(&receipt) {
            self.clear_pending();
            return Ok(normalize(receipt));
        }
        // Verification only: the transport re-reads the target and settles the
        // receipt when it can prove the outcome. It never dispatches again.
        let released = self.transport.release(operation_id)?;
        if settled(&released) {
            self.clear_pending();
            return Ok(normalize(released));
        }
        Err(dispatch_error(
            "Native dispatch result is uncertain; allocation retained, do not retry",
        ))
    }

    fn require_process_instance(
        &mut self,
        pid: u32,
        process_creation_time: Option<&str>,
    ) -> Result<(), RuntimeError> {
        self.require_instance(pid, process_creation_time)
    }

    /// `LiveAddAdapter.submission_absent`: local knowledge only.
    fn submission_absent(&mut self, operation_id: &str) -> bool {
        !self.transport.operation_known(operation_id)
    }

    fn safe_to_shutdown(&mut self) -> bool {
        self.refresh_pending();
        self.pending.is_none() && self.transport.safe_to_shutdown()
    }
}

/// A bounded wait budget, so tests never sleep and the product can.
#[derive(Debug, Clone, Copy)]
pub struct DispatchBudget {
    pub polls: u32,
    pub pause: fn(),
}

impl DispatchBudget {
    /// The shipped `wait`: 15 s at a 50 ms poll.
    pub const PRODUCT: Self = Self {
        polls: 300,
        pause: product_pause,
    };
}

fn product_pause() {
    std::thread::sleep(std::time::Duration::from_millis(50));
}

/// `LiveAddAdapter.normalize`: a zero breakpoint count becomes an empty list.
pub fn normalize(value: Value) -> Value {
    let mut value = value;
    if let Some(object) = value.as_object_mut() {
        let count = object
            .remove("breakpoint_count")
            .and_then(|value| value.as_i64())
            .unwrap_or(-1);
        object.insert(
            "breakpoints".to_string(),
            if count == 0 {
                json!([])
            } else {
                json!(["unconfirmed"])
            },
        );
    }
    value
}

/// `live_add_profile` fields an inventory capture resolves addresses with.
pub fn inventory_layout(layout: &LiveAddLayout) -> InventoryLayout {
    InventoryLayout {
        insertion_rva: layout.insertion_rva,
        manager_pointer_rva: layout.manager_pointer_rva,
        container_offset: layout.container_offset,
        capacity_offset: layout.capacity_offset,
        serial_index_offset: layout.serial_index_offset,
        capacity: layout.capacity,
        record_size: layout.record_size,
        serial_counter_offset: layout.serial_counter_offset,
    }
}

fn read_u64<T: LiveAddTransport + ?Sized>(
    transport: &mut T,
    address: u64,
) -> Result<u64, RuntimeError> {
    let raw = transport.read(address, 8)?;
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&raw);
    Ok(u64::from_le_bytes(bytes))
}

/// The durable receipt store: `<operation_id>.json`, written atomically.
pub struct ReceiptStore {
    pub directory: PathBuf,
}

impl ReceiptStore {
    pub fn new(directory: &Path) -> Result<Self, RuntimeError> {
        std::fs::create_dir_all(directory).map_err(|error| RuntimeError::Io {
            path: directory.display().to_string(),
            detail: error.to_string(),
        })?;
        Ok(Self {
            directory: directory.to_path_buf(),
        })
    }

    pub fn path(&self, operation_id: &str) -> PathBuf {
        self.directory.join(format!("{operation_id}.json"))
    }

    /// `NativeLiveAddTransport._save`: a temporary file, then a rename.
    pub fn save(&self, receipt: &Value) -> Result<(), RuntimeError> {
        let operation_id = receipt
            .get("operation_id")
            .and_then(Value::as_str)
            .ok_or_else(|| rejected("Native receipt has no operation identity"))?;
        let path = self.path(operation_id);
        let temporary = path.with_extension("json.tmp");
        let text = serde_json::to_string(receipt).map_err(|error| RuntimeError::Io {
            path: temporary.display().to_string(),
            detail: error.to_string(),
        })?;
        std::fs::write(&temporary, text.as_bytes()).map_err(|error| RuntimeError::Io {
            path: temporary.display().to_string(),
            detail: error.to_string(),
        })?;
        std::fs::rename(&temporary, &path).map_err(|error| RuntimeError::Io {
            path: path.display().to_string(),
            detail: error.to_string(),
        })
    }

    pub fn read(&self, operation_id: &str) -> Result<Value, RuntimeError> {
        read_json(&self.path(operation_id))
    }

    pub fn exists(&self, operation_id: &str) -> bool {
        self.path(operation_id).is_file()
    }

    /// `_unresolved_owner`: every receipt that still owns the target.
    pub fn unresolved_owner(&self) -> Result<Option<String>, RuntimeError> {
        for value in self.all()? {
            if settled(&value) {
                continue;
            }
            return Ok(Some(
                value
                    .get("operation_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ));
        }
        Ok(None)
    }

    /// Every receipt in the directory, in file-name order.
    pub fn all(&self) -> Result<Vec<Value>, RuntimeError> {
        let entries = std::fs::read_dir(&self.directory).map_err(|error| RuntimeError::Io {
            path: self.directory.display().to_string(),
            detail: error.to_string(),
        })?;
        let mut values = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| RuntimeError::Io {
                path: self.directory.display().to_string(),
                detail: error.to_string(),
            })?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let value = read_json(&path)?;
            if value.get("pid").and_then(Value::as_u64).is_none() {
                return Err(rejected(format!(
                    "Unresolved native receipt {}: Missing receipt process identity",
                    path.display()
                )));
            }
            values.push(value);
        }
        Ok(values)
    }
}

/// `<directory>/admission.lock`, held only while one operation is admitted.
pub struct AdmissionLock {
    path: PathBuf,
    held: bool,
}

impl AdmissionLock {
    pub fn acquire(directory: &Path) -> Result<Self, RuntimeError> {
        let path = directory.join("admission.lock");
        std::fs::create_dir_all(directory).map_err(|error| RuntimeError::Io {
            path: directory.display().to_string(),
            detail: error.to_string(),
        })?;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        match options.open(&path) {
            Ok(_) => Ok(Self { path, held: true }),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Err(rejected(
                "Another native executor is admitting an operation",
            )),
            Err(error) => Err(RuntimeError::Io {
                path: path.display().to_string(),
                detail: error.to_string(),
            }),
        }
    }
}

impl Drop for AdmissionLock {
    fn drop(&mut self) {
        if self.held {
            let _ = std::fs::remove_file(&self.path);
            self.held = false;
        }
    }
}

/// `LiveAddApplication` persistence check against the decrypted save.
pub fn save_sha256(path: &Path) -> Result<String, RuntimeError> {
    Ok(sha256_hex(&read_bytes(path)?))
}

/// The candidate transfer type, re-exported for callers that build a plan
/// without the application.
pub type ExecutorCandidate = InstallationCandidate;

/// Stable ordering helper the batch uses when it reports receipts.
pub fn ordered_receipts(receipts: &BTreeMap<String, Value>) -> Vec<Value> {
    receipts.values().cloned().collect()
}

/// Unused imports that document the shim's own contract addresses.
#[allow(dead_code)]
fn _shim_offsets() -> [u64; 7] {
    [
        DISPATCH_MARKER_OFFSET,
        BUILDER_RESULT_OFFSET,
        DISPATCH_STATUS_OFFSET,
        DISPATCH_SLOT_OFFSET,
        INSERTION_RESULT_OFFSET,
        DISPATCH_DESCRIPTOR_OFFSET,
        DISPATCH_REMAINDER_OFFSET,
    ]
}

#[cfg(windows)]
mod windows_transport {
    use super::*;
    use crate::mutation::win_session::{
        DebugSession, WindowsDebugSession, EXCEPTION_BREAKPOINT, EXCEPTION_SINGLE_STEP,
    };

    /// How long the dispatch loop waits for the accepted idle window.
    const DISPATCH_WAIT_ROUNDS: u32 = 100;

    /// The real transport: one `WindowsDebug` session per accepted dispatch.
    pub struct NativeDebugTransport {
        pid: u32,
        layout: LiveAddLayout,
        module_name: String,
        store: ReceiptStore,
        /// The allocation an uncertain redirect still owns.
        retained: Option<u64>,
    }

    impl NativeDebugTransport {
        pub fn new(
            pid: u32,
            layout: LiveAddLayout,
            module_name: &str,
            directory: &Path,
        ) -> Result<Self, RuntimeError> {
            Ok(Self {
                pid,
                layout,
                module_name: module_name.to_string(),
                store: ReceiptStore::new(directory)?,
                retained: None,
            })
        }

        pub fn receipts(&self) -> &ReceiptStore {
            &self.store
        }

        fn open(&self) -> Result<WindowsDebugSession, RuntimeError> {
            WindowsDebugSession::open(self.pid, &self.module_name)
        }

        /// Verification-only settlement of a receipt that never acknowledged.
        fn verify_outcome(
            &mut self,
            receipt: &mut Value,
            session: &mut WindowsDebugSession,
        ) -> Result<bool, RuntimeError> {
            let layout = self.layout;
            let base = session.module_base()?;
            let manager = read_session_u64(session, base + layout.manager_pointer_rva)?;
            let data = read_session_u64(session, manager)?;
            if manager == 0 || data == 0 {
                return Ok(false);
            }
            let serial = read_session_u64(session, data + layout.serial_counter_offset)?;
            let slot = receipt.get("slot").and_then(Value::as_u64);
            let planned = receipt.get("serial").and_then(Value::as_u64);
            let source = receipt
                .get("source_hex")
                .and_then(Value::as_str)
                .map(hex_decode)
                .transpose()?;
            let container = data + layout.container_offset;
            if let (Some(slot), Some(planned), Some(source)) = (slot, planned, source) {
                let destination =
                    session.read(container + slot * layout.record_size as u64, RECORD_SIZE)?;
                // A present destination record plus the advanced counter proves
                // the insertion happened; the shipment is resolved, not replayed.
                if destination[0] == source[0]
                    && destination[1] == source[1]
                    && destination[0x28..0x30] == source[0x28..0x30]
                    && serial > planned
                {
                    settle(receipt, "completed", true);
                    return Ok(true);
                }
            }
            if let (Some(slot), Some(planned)) = (slot, planned) {
                let destination =
                    session.read(container + slot * layout.record_size as u64, RECORD_SIZE)?;
                if destination[0] == 0 && destination[1] == 0 && serial == planned {
                    // The destination is still empty and the serial did not
                    // advance: the dispatch provably did not reach insertion.
                    settle(receipt, "rejected", false);
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }

    fn settle(receipt: &mut Value, phase: &str, completed: bool) {
        if let Some(object) = receipt.as_object_mut() {
            object.insert("phase".to_string(), json!(phase));
            object.insert("active".to_string(), json!(false));
            object.insert("released".to_string(), json!(true));
            object.insert("breakpoint_count".to_string(), json!(0));
            if completed {
                object.insert(
                    "recovered_by".to_string(),
                    json!("destination_record_and_serial"),
                );
            } else {
                object.insert("recovered_by".to_string(), json!("proven_absence"));
            }
        }
    }

    fn read_session_u64(
        session: &mut WindowsDebugSession,
        address: u64,
    ) -> Result<u64, RuntimeError> {
        let raw = session.read(address, 8)?;
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&raw);
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_session_u32(
        session: &mut WindowsDebugSession,
        address: u64,
    ) -> Result<u32, RuntimeError> {
        let raw = session.read(address, 4)?;
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&raw);
        Ok(u32::from_le_bytes(bytes))
    }

    impl LiveAddTransport for NativeDebugTransport {
        fn pid(&self) -> u32 {
            self.pid
        }

        fn profile_id(&self) -> &'static str {
            self.layout.profile_id
        }

        fn module_base(&mut self) -> Result<u64, RuntimeError> {
            let mut session = self.open()?;
            session.module_base()
        }

        fn creation_time(&mut self) -> Result<String, RuntimeError> {
            let mut session = self.open()?;
            session.creation_time()
        }

        fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
            let mut session = self.open()?;
            session.read(address, size)
        }

        fn ping(&mut self) -> Result<Value, RuntimeError> {
            Ok(json!({
                "pid": self.pid,
                "profile_id": self.layout.profile_id,
                "busy": self.retained.is_some() || self.store.unresolved_owner()?.is_some(),
            }))
        }

        fn dispatch(&mut self, mode: DispatchMode, params: &Value) -> Result<Value, RuntimeError> {
            let operation_id = params
                .get("operation_id")
                .and_then(Value::as_str)
                .ok_or_else(|| rejected("Native admission needs an operation identity"))?
                .to_string();
            if self.store.exists(&operation_id) || self.retained.is_some() {
                return Err(dispatch_error(
                    "Operation already submitted or executor is occupied",
                ));
            }
            let _admission = AdmissionLock::acquire(&self.store.directory)?;
            if let Some(unresolved) = self.store.unresolved_owner()? {
                return Err(dispatch_error(format!(
                    "Previous native operation {unresolved} is unresolved; recover it, never replay"
                )));
            }
            if self.store.exists(&operation_id) {
                return Err(dispatch_error(
                    "Operation already submitted or executor is occupied",
                ));
            }
            let mut session = self.open()?;
            let creation = session.creation_time()?;
            if params.get("process_creation_time").and_then(Value::as_str)
                != Some(creation.as_str())
            {
                return Err(dispatch_error(
                    "PROCESS_INSTANCE_CHANGED: native admission refused",
                ));
            }
            let mut receipt = json!({
                "operation_id": operation_id,
                "pid": self.pid,
                "process_creation_time": creation,
                "phase": "preparing",
                "active": true,
                "released": false,
                "redirect_count": 0,
                "breakpoint_count": -1,
                "executor": "windows-native",
                "mode": mode.receipt_mode(),
                "source_save_path": params.get("source_save_path").cloned().unwrap_or(Value::Null),
                "candidate_id": params.get("candidate_id").cloned().unwrap_or(Value::Null),
                "parent_operation_id": params.get("parent_operation_id").cloned().unwrap_or(Value::Null),
                "expected_record_hex": params.get("expected_record_hex").cloned().unwrap_or(Value::Null),
                "serial": params.get("serial").cloned().unwrap_or(Value::Null),
                "slot": params.get("slot").cloned().unwrap_or(Value::Null),
            });
            self.store.save(&receipt)?;
            let outcome = run_dispatch(
                &mut session,
                &self.store,
                &self.layout,
                mode,
                params,
                &mut receipt,
            );
            match outcome {
                Ok(()) => {
                    // The shim always leaves a settled receipt behind.
                    let stored = self.store.read(receipt_operation_id(&receipt)?)?;
                    if settled(&stored) {
                        self.retained = None;
                        return Ok(stored);
                    }
                    self.retained = receipt.get("allocation").and_then(Value::as_u64);
                    Err(dispatch_error(
                        "Native dispatch result is uncertain; allocation retained, do not retry",
                    ))
                }
                Err(error) => {
                    let stored = self.store.read(&operation_id)?;
                    if settled(&stored) {
                        return Err(error);
                    }
                    self.retained = stored.get("allocation").and_then(Value::as_u64);
                    Err(error)
                }
            }
        }

        fn status(&mut self, operation_id: &str) -> Result<Value, RuntimeError> {
            if self.store.exists(operation_id) {
                return self.store.read(operation_id);
            }
            Err(dispatch_error("Unknown native operation"))
        }

        fn release(&mut self, operation_id: &str) -> Result<Value, RuntimeError> {
            let mut value = self.store.read(operation_id)?;
            if settled(&value) {
                return Ok(value);
            }
            // Verification only: one read view decides the outcome, and the
            // receipt is rewritten in place. Nothing is dispatched.
            let mut session = self.open()?;
            if self.verify_outcome(&mut value, &mut session)? {
                self.store.save(&value)?;
                self.retained = None;
                return Ok(value);
            }
            let phase = value
                .get("phase")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            Err(dispatch_error(format!(
                "Previous executor ownership is unresolved for {}; never replay this operation",
                phase
            )))
        }

        fn operation_known(&mut self, operation_id: &str) -> bool {
            self.store.exists(operation_id)
        }

        fn safe_to_shutdown(&mut self) -> bool {
            self.retained.is_none()
                && self
                    .store
                    .unresolved_owner()
                    .map(|owner| owner.is_none())
                    .unwrap_or(false)
        }
    }

    fn receipt_operation_id(receipt: &Value) -> Result<&str, RuntimeError> {
        receipt
            .get("operation_id")
            .and_then(Value::as_str)
            .ok_or_else(|| rejected("Native receipt has no operation identity"))
    }

    /// The shipped `_run` loop, in the synchronous shape this crate publishes:
    /// one dispatch, one durable receipt, no replay. It returns `Ok(())` when
    /// the receipt is settled either way and `Err` when ownership is retained.
    fn run_dispatch(
        session: &mut WindowsDebugSession,
        store: &ReceiptStore,
        layout: &LiveAddLayout,
        mode: DispatchMode,
        params: &Value,
        receipt: &mut Value,
    ) -> Result<(), RuntimeError> {
        let mut allocation: Option<u64> = None;
        let mut redirected = false;
        let mut acknowledged = false;
        let mut failure: Option<RuntimeError> = None;

        let outcome: Result<(), RuntimeError> = (|| {
            let base = session.module_base()?;
            let entry = base + layout.dispatch_rva;
            let target = entry + 7;
            let original = layout.dispatch_signature.to_vec();
            if session.read(entry, original.len())? != original {
                return Err(dispatch_error(format!(
                    "Native precondition changed at {entry:#x}"
                )));
            }
            let manager = read_session_u64(session, base + layout.manager_pointer_rva)?;
            let data = read_session_u64(session, manager)?;
            if manager == 0 || data == 0 {
                return Err(dispatch_error("Inventory owner is not loaded"));
            }
            let container = data + layout.container_offset;
            let mut expected_record = None;
            let descriptor = if mode == DispatchMode::Noop {
                None
            } else {
                let descriptor = hex_decode(
                    params
                        .get("descriptor_hex")
                        .and_then(Value::as_str)
                        .ok_or_else(|| rejected("Incomplete assembly input"))?,
                )?;
                let expected = hex_decode(
                    params
                        .get("expected_record_hex")
                        .and_then(Value::as_str)
                        .ok_or_else(|| rejected("Incomplete assembly input"))?,
                )?;
                if descriptor.len() != layout.descriptor_size
                    || expected.len() != layout.record_size
                {
                    return Err(rejected("Incomplete assembly input"));
                }
                expected_record = Some(expected);
                Some(descriptor)
            };
            if mode == DispatchMode::Insert {
                if params.get("manager").and_then(Value::as_u64) != Some(manager)
                    || params.get("data").and_then(Value::as_u64) != Some(data)
                    || params.get("function_address").and_then(Value::as_u64)
                        != Some(base + layout.insertion_rva)
                {
                    return Err(rejected("Insertion targets changed"));
                }
                let slot = params
                    .get("slot")
                    .and_then(Value::as_u64)
                    .unwrap_or(u64::MAX);
                if slot >= u64::from(layout.capacity) {
                    return Err(rejected("Invalid destination slot"));
                }
                let builder = hex_decode(
                    params
                        .get("builder_code_hex")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                )?;
                if session.read(base + layout.builder_rva, builder.len())? != builder {
                    return Err(dispatch_error(format!(
                        "Native precondition changed at {:#x}",
                        base + layout.builder_rva
                    )));
                }
                let insertion = hex_decode(
                    params
                        .get("insertion_code_hex")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                )?;
                if session.read(base + layout.insertion_rva, insertion.len())? != insertion {
                    return Err(dispatch_error(format!(
                        "Native precondition changed at {:#x}",
                        base + layout.insertion_rva
                    )));
                }
                let container_bytes = hex_decode(
                    params
                        .get("container_hex")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                )?;
                if session.read(container, container_bytes.len())? != container_bytes {
                    return Err(dispatch_error(format!(
                        "Native precondition changed at {container:#x}"
                    )));
                }
            }
            let address = session.allocate(REMOTE_CODE_SIZE as usize)?;
            allocation = Some(address);
            let leaf = if mode == DispatchMode::Noop {
                None
            } else {
                Some(base + layout.builder_rva)
            };
            let insertion_args = if mode == DispatchMode::Insert {
                Some(InsertionArgs {
                    serial: params.get("serial").and_then(Value::as_u64).unwrap_or(0),
                    data,
                    manager,
                    function_address: base + layout.insertion_rva,
                    serial_counter_offset: layout.serial_counter_offset,
                })
            } else {
                None
            };
            let preserve_rarity5 = expected_record
                .as_ref()
                .map(|expected| expected.get(0x30..0x32) == Some(&[0x05, 0x05][..]))
                .unwrap_or(false);
            let code = build_dispatch_code(
                address,
                target,
                &original,
                leaf,
                address + DISPATCH_SOURCE_OFFSET,
                if mode == DispatchMode::Noop {
                    None
                } else {
                    Some(address + DISPATCH_DESCRIPTOR_OFFSET)
                },
                insertion_args,
                preserve_rarity5,
            )?;
            session.write(address, &vec![0u8; 4096])?;
            session.write(address, &code)?;
            if let Some(descriptor) = &descriptor {
                session.write(address + DISPATCH_DESCRIPTOR_OFFSET, descriptor)?;
                for canary in DISPATCH_CANARIES {
                    session.write(address + canary, &[0xA5u8; 16])?;
                }
                session.write(address + DISPATCH_SLOT_OFFSET, &[0xFFu8; 4])?;
            }
            if session.read(address, code.len())? != code {
                return Err(dispatch_error("Native allocation did not accept the shim"));
            }
            session.flush_instruction_cache(address, code.len())?;
            session.attach()?;
            let planned_creation = params
                .get("process_creation_time")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if session.creation_time()? != planned_creation {
                return Err(dispatch_error(
                    "PROCESS_INSTANCE_CHANGED: game changed during debugger attach",
                ));
            }
            let mut armed = false;
            let mut chosen = None;
            let mut rounds = 0;
            while chosen.is_none() {
                rounds += 1;
                if rounds > DISPATCH_WAIT_ROUNDS {
                    failure = Some(dispatch_error("No accepted idle dispatch before timeout"));
                    break;
                }
                let Some(event) = session.wait(100)? else {
                    if rounds > 20 && !redirected {
                        // Nudge the mission thread once the idle window is late.
                        session.debug_break()?;
                    }
                    continue;
                };
                if event.is_exit_process() {
                    allocation = None;
                    return Err(dispatch_error("Game exited during native dispatch"));
                }
                if event.is_create_thread() || event.is_create_process() {
                    if let Some(handle) = event.thread_handle {
                        session.adopt_thread(event.tid, handle)?;
                        session.arm_thread(event.tid, entry, target)?;
                    }
                    session.resume(&event, true)?;
                    continue;
                }
                if event.is_load_dll() {
                    session.resume(&event, true)?;
                    continue;
                }
                if event.is_exit_thread() {
                    session.resume(&event, true)?;
                    continue;
                }
                if !event.is_exception() {
                    session.resume(&event, true)?;
                    continue;
                }
                let code = event.exception_code.unwrap_or_default();
                if code == EXCEPTION_BREAKPOINT {
                    session.resume(&event, true)?;
                    if !armed {
                        armed = true;
                        if let Some(object) = receipt.as_object_mut() {
                            object.insert("phase".to_string(), json!("armed"));
                        }
                        store.save(receipt)?;
                    }
                    continue;
                }
                if code != EXCEPTION_SINGLE_STEP {
                    session.resume(&event, true)?;
                    continue;
                }
                let mut context = session.context(event.tid)?;
                if context.rip == entry && context.dr6 & 1 != 0 {
                    context.dr6 &= !1;
                    context.eflags |= 0x10000;
                    if !redirected {
                        // The accepted idle window: recheck every owner before
                        // the claim becomes durable.
                        let manager_now =
                            read_session_u64(session, base + layout.manager_pointer_rva)?;
                        let data_now = read_session_u64(session, manager_now)?;
                        if manager_now != manager || data_now != data {
                            context.rip = entry;
                            session.set_context(event.tid, &context)?;
                            session.resume(&event, true)?;
                            continue;
                        }
                        if mode == DispatchMode::Insert {
                            let serial =
                                read_session_u64(session, data + layout.serial_counter_offset)?;
                            let scheduler =
                                read_session_u64(session, base + layout.scheduler_pointer_rva)?;
                            let serial_matches = params
                                .get("serial")
                                .and_then(Value::as_u64)
                                .map(|planned| planned == serial)
                                .unwrap_or(true);
                            let scheduler_matches = params
                                .get("scheduler_owner")
                                .and_then(Value::as_u64)
                                .map(|planned| planned == scheduler)
                                .unwrap_or(true);
                            if !serial_matches || !scheduler_matches {
                                failure =
                                    Some(dispatch_error("Serial or scheduler ownership changed"));
                                break;
                            }
                        }
                        if let Some(object) = receipt.as_object_mut() {
                            object.insert("phase".to_string(), json!("redirected"));
                            object.insert("redirect_count".to_string(), json!(1));
                            object.insert("before".to_string(), json!(context.registers()));
                            object.insert("thread_id".to_string(), json!(event.tid));
                            object.insert("allocation".to_string(), json!(address));
                        }
                        store.save(receipt)?;
                        redirected = true;
                        context.rip = address;
                    }
                    session.set_context(event.tid, &context)?;
                    session.resume(&event, true)?;
                    continue;
                }
                if context.rip == target && context.dr6 & 2 != 0 {
                    context.dr6 &= !2;
                    context.eflags |= 0x10000;
                    session.set_context(event.tid, &context)?;
                    let status = read_session_u32(session, address + DISPATCH_STATUS_OFFSET)?;
                    let source = session.read(address + DISPATCH_SOURCE_OFFSET, RECORD_SIZE)?;
                    if let Some(object) = receipt.as_object_mut() {
                        object.insert("after".to_string(), json!(context.registers()));
                        object.insert("source_hex".to_string(), json!(hex(&source)));
                        object.insert("status".to_string(), json!(status));
                    }
                    if let Some(expected) = &expected_record {
                        let planned_serial = params
                            .get("serial")
                            .and_then(Value::as_u64)
                            .unwrap_or(u64::MAX);
                        let wanted = if mode == DispatchMode::Insert {
                            planned_serial
                        } else {
                            u64::MAX
                        };
                        let actual_serial =
                            u64::from_le_bytes(source[0x28..0x30].try_into().unwrap_or_default());
                        let differs = source[..0x24] != expected[..0x24]
                            || source[0x30..0xE4] != expected[0x30..0xE4]
                            || actual_serial != wanted;
                        if differs {
                            failure = Some(dispatch_error(
                                "Native builder output differs from reviewed record",
                            ));
                        }
                    }
                    if mode == DispatchMode::Insert {
                        let slot = read_session_u32(session, address + DISPATCH_SLOT_OFFSET)?;
                        let remainder =
                            session.read(address + DISPATCH_REMAINDER_OFFSET, RECORD_SIZE)?;
                        let destination = session.read(
                            container + u64::from(slot) * layout.record_size as u64,
                            RECORD_SIZE,
                        )?;
                        if let Some(object) = receipt.as_object_mut() {
                            object.insert("slot".to_string(), json!(slot));
                            object.insert("remainder_hex".to_string(), json!(hex(&remainder)));
                            object.insert("destination_hex".to_string(), json!(hex(&destination)));
                        }
                    }
                    acknowledged = true;
                    chosen = Some(event.tid);
                    session.resume(&event, true)?;
                    continue;
                }
                session.set_context(event.tid, &context)?;
                session.resume(&event, true)?;
            }
            session.restore_threads()?;
            if let Some(address) = allocation {
                if !redirected || acknowledged {
                    session.free(address)?;
                    allocation = None;
                }
            }
            session.detach()?;
            Ok(())
        })();

        let error_text = match (&outcome, &failure) {
            (Err(error), _) => Some(error.message()),
            (Ok(()), Some(error)) => Some(error.message()),
            (Ok(()), None) => None,
        };
        let completed = acknowledged && error_text.is_none() && allocation.is_none();
        if let Some(object) = receipt.as_object_mut() {
            object.insert(
                "phase".to_string(),
                json!(if completed {
                    "completed"
                } else if redirected {
                    "uncertain"
                } else {
                    "rejected"
                }),
            );
            object.insert("error".to_string(), json!(error_text));
            object.insert("active".to_string(), json!(false));
            object.insert("released".to_string(), json!(allocation.is_none()));
            object.insert(
                "breakpoint_count".to_string(),
                json!(if allocation.is_none() { 0 } else { -1 }),
            );
            object.insert(
                "allocation".to_string(),
                match allocation {
                    Some(address) => json!(address),
                    None => Value::Null,
                },
            );
        }
        store.save(receipt)?;
        if allocation.is_some() {
            return Err(dispatch_error(
                "Native dispatch result is uncertain; allocation retained, do not retry",
            ));
        }
        match (outcome, failure) {
            (Err(error), _) => Err(error),
            (Ok(()), Some(error)) => Err(error),
            (Ok(()), None) => Ok(()),
        }
    }
}

#[cfg(windows)]
pub use windows_transport::NativeDebugTransport;
