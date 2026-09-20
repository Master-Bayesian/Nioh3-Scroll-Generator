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
//! * `recover` verifies, it never dispatches;
//! * a durable receipt is a claim, not a release proof: `pending` clears and a
//!   recovery reports success only when the transport also proves that no
//!   allocation or debug session is still retained.

use crate::error::RuntimeError;
use crate::mutation::count::{new_operation_id, read_bytes, read_json, sha256_hex};
use crate::mutation::descriptor::{assembly_descriptor, verify_assembly_preview};
use crate::mutation::evidence::verify_dispatch;
use crate::mutation::inventory::{
    capture_read_only, hex_decode, Inventory, InventoryLayout, InventoryProcess, NativeIndex,
    RECORD_SIZE,
};
use crate::mutation::live_add::{InstallationCandidate, LiveAddExecutor};
use crate::mutation::native_abi::{
    build_dispatch_code, hex, InsertionArgs, LiveAddLayout, BUILDER_RESULT_OFFSET,
    CANDIDATE_DISPLAY_VERSION, DISPATCH_CANARIES, DISPATCH_DESCRIPTOR_OFFSET,
    DISPATCH_MARKER_OFFSET, DISPATCH_REMAINDER_OFFSET, DISPATCH_SLOT_OFFSET,
    DISPATCH_SOURCE_OFFSET, DISPATCH_STATUS_OFFSET, INSERTION_RESULT_OFFSET, PC_V201_LIVE_ADD,
    PC_V202_CANDIDATE_EXECUTABLE_SHA256, PC_V202_LIVE_ADD_CANDIDATE, PRODUCT_DISPLAY_VERSION,
    REMOTE_CODE_SIZE,
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

/// The highest generation serial the save format can represent, mirroring
/// `nioh3_save::codec::SCROLL_GENERATION_SERIAL_MAX`.
///
/// The live record's `+0x28` is a qword and the native index hashes all eight
/// bytes, but the saved field is a u32 capped here. The insertion stamps the
/// planned serial into the new record and advances the live counter to
/// `serial + 1`, so a counter whose successor passes this cap cannot round-trip
/// into a save. The executor refuses such a plan instead of truncating the live
/// value or widening the save field.
pub const SAVE_GENERATION_SERIAL_MAX: u64 = 0xFFFF_FFFC;

fn dispatch_error(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::NativeDispatch {
        detail: detail.into(),
    }
}

/// The stable refusal a recovery reports while a runtime owner is retained.
///
/// A receipt that records a resolved `business_outcome` while `released` is
/// false is evidence about the insertion and never about the cleanup owner, so
/// the refusal names both facts: an operator can tell "the write landed" from
/// "the debugger, the allocation or a thread owner is still ours", and the
/// wording stays stable enough for a test to key on.
fn retained_owner_error(operation_id: &str, receipt: &Value) -> RuntimeError {
    let business = receipt
        .get("business_outcome")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let allocation = receipt
        .get("allocation_state")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    dispatch_error(format!(
        "Native recovery refused: runtime owner still retained for {operation_id} \
         (business_outcome={business}, allocation_state={allocation}); a receipt is not \
         release proof, do not retry"
    ))
}

/// The per-thread cleanup states that mean "this record is finished".
///
/// A create-event handle leaves our ownership three ways: its debug registers
/// were restored (`original_restored`), the thread exited under our ownership
/// (`exited`), or the record was never armed (`not_armed`). A record Windows
/// closed, terminated, or handed the numeric value to another thread is retired
/// as `handle_reused` (`win_session.rs`): the owner is gone and the record is
/// never written through again, so it is exactly as complete as `exited` and
/// must release the same way. Retained (`armed`, `stopped`) and unresolved
/// (`restore_failed`, `unknown`) states are deliberately absent.
const COMPLETED_CLEANUP_STATES: [&str; 4] =
    ["original_restored", "exited", "not_armed", "handle_reused"];

/// `released`, `inactive` and zero breakpoints: the shipped settled predicate.
pub fn settled(value: &Value) -> bool {
    value.get("released").and_then(Value::as_bool) == Some(true)
        && value.get("active").and_then(Value::as_bool) == Some(false)
        && value.get("breakpoint_count").and_then(Value::as_i64) == Some(0)
        && value
            .get("business_outcome")
            .and_then(Value::as_str)
            .is_some_and(|state| matches!(state, "committed" | "completed" | "rejected"))
        && value
            .get("remote_execution")
            .and_then(Value::as_str)
            .is_some_and(|state| matches!(state, "quiescent" | "not_started"))
        && value
            .get("allocation_state")
            .and_then(Value::as_str)
            .is_some_and(|state| matches!(state, "freed" | "not_allocated"))
        && value
            .get("debugger_state")
            .and_then(Value::as_str)
            .is_some_and(|state| matches!(state, "detached" | "not_attached"))
        && value
            .get("thread_cleanup")
            .and_then(Value::as_object)
            .is_some_and(|threads| {
                threads.values().all(|thread| {
                    thread
                        .get("cleanup_state")
                        .and_then(Value::as_str)
                        .is_some_and(|state| COMPLETED_CLEANUP_STATES.contains(&state))
                })
            })
}

/// `live_add_profile.live_add_profile((2, 0, 1, 0))` acceptance.
pub const REQUIRED_DISPLAY_VERSION: &str = PRODUCT_DISPLAY_VERSION;

/// One accepted `(layout, display version, exact executable)` binding.
///
/// The pair is the binding: a display version alone never authorizes a layout,
/// and a layout alone never authorizes a version. `executable_sha256` is `None`
/// where the accepted layout plus the verified dispatch-signature read is the
/// authority, and `Some` where the run must also prove the exact executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveAddBinding {
    pub layout: &'static LiveAddLayout,
    pub display_version: &'static str,
    pub executable_sha256: Option<&'static str>,
}

/// The only accepted bindings: the shipped product build and the pinned PC v2.02
/// research candidate.
///
/// The candidate entry is opt-in research. Nothing in the product names it, and
/// a run that selects it must also make the transport prove
/// [`PC_V202_CANDIDATE_EXECUTABLE_SHA256`] before any read or dispatch.
pub const ACCEPTED_LIVE_ADD_BINDINGS: [LiveAddBinding; 2] = [
    LiveAddBinding {
        layout: &PC_V201_LIVE_ADD,
        display_version: PRODUCT_DISPLAY_VERSION,
        executable_sha256: None,
    },
    LiveAddBinding {
        layout: &PC_V202_LIVE_ADD_CANDIDATE,
        display_version: CANDIDATE_DISPLAY_VERSION,
        executable_sha256: Some(PC_V202_CANDIDATE_EXECUTABLE_SHA256),
    },
];

/// The accepted binding a `(layout, display version)` pair names, if any.
pub fn accepted_live_add_binding(
    layout: &LiveAddLayout,
    display_version: &str,
) -> Option<LiveAddBinding> {
    ACCEPTED_LIVE_ADD_BINDINGS
        .iter()
        .copied()
        .find(|binding| binding.layout == layout && binding.display_version == display_version)
}

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

    /// Whether this transport still owns the allocation or the debug session
    /// of an admitted native operation.
    ///
    /// The adapter never treats a durable receipt as proof: a record can read
    /// `released` while the process that owns the target is still alive (its
    /// durable entry was deregistered before the owner finished). Only this
    /// answer, together with a settled receipt, releases `pending`, and a
    /// transport that cannot prove release must answer `true`.
    fn owner_retained(&mut self) -> bool;

    /// Exact SHA-256 of the attached executable, upper-case hex, when the
    /// transport can prove it.
    ///
    /// The shipped PC v2.01 binding never asks: its authority is the accepted
    /// layout plus the verified dispatch-signature read. A binding that names an
    /// exact executable refuses to read or dispatch until this matches, so a
    /// transport that cannot prove the identity fails closed with `None`.
    fn executable_sha256(&mut self) -> Result<Option<String>, RuntimeError> {
        Ok(None)
    }
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

    /// Research-only: the pinned PC v2.02 candidate binding.
    ///
    /// This is not product selection. The executor still refuses unless the
    /// transport advertises the candidate profile id and proves the exact pinned
    /// executable identity, and no product caller names it.
    pub fn candidate(transport: T) -> Self {
        Self::new(
            transport,
            PC_V202_LIVE_ADD_CANDIDATE,
            CANDIDATE_DISPLAY_VERSION,
        )
    }

    /// Override the wait budget; tests use a zero-poll budget.
    pub fn with_budget(mut self, budget: DispatchBudget) -> Self {
        self.budget = budget;
        self
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// The transport, for a caller that must inspect or drive its own state.
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn layout(&self) -> &LiveAddLayout {
        &self.layout
    }

    /// `LiveAddAdapter.identity`: a typed refusal before anything is opened.
    ///
    /// The accepted binding is the authority. The shipped product pair keeps its
    /// exact message and behaviour; the candidate pair is accepted only when the
    /// transport advertises the candidate profile id and proves the pinned
    /// executable identity, so a v2.01 layout with the candidate version (or the
    /// inverse) is refused before any read or dispatch.
    fn require_accepted_binding(&mut self) -> Result<&'static str, RuntimeError> {
        let candidate_run = self.layout == PC_V202_LIVE_ADD_CANDIDATE
            || self.display_version == CANDIDATE_DISPLAY_VERSION;
        let refusal = |detail: &'static str| dispatch_error(detail);
        let Some(binding) = accepted_live_add_binding(&self.layout, &self.display_version) else {
            return Err(if candidate_run {
                refusal("Candidate live addition requires the pinned PC v2.02 layout and version")
            } else {
                refusal("Live addition requires accepted PC v2.01")
            });
        };
        if self.transport.profile_id() != binding.layout.profile_id {
            return Err(if candidate_run {
                refusal("Candidate live addition requires the pinned PC v2.02 executable")
            } else {
                refusal("Live addition requires accepted PC v2.01")
            });
        }
        if let Some(expected) = binding.executable_sha256 {
            if self.transport.executable_sha256()?.as_deref() != Some(expected) {
                return Err(refusal(
                    "Candidate live addition requires the pinned PC v2.02 executable",
                ));
            }
        }
        Ok(binding.display_version)
    }

    /// `LiveAddAdapter._refresh_pending`.
    fn refresh_pending(&mut self) {
        let Some(pending) = self.pending.clone() else {
            return;
        };
        if let Ok(value) = self.transport.status(&pending) {
            if value.get("operation_id").and_then(Value::as_str) == Some(pending.as_str())
                && self.owner_released(&value)
            {
                self.clear_pending();
            }
        }
    }

    /// A proven terminal owner: the durable receipt settles *and* the transport
    /// proves it holds no allocation or debug session for it.
    ///
    /// A receipt is a claim on disk. A reaper that deregisters its entry before
    /// its window closes leaves a record that reads `released` while the target
    /// is still owned, so a receipt alone never clears `pending` and never lets
    /// a recovery report success.
    fn owner_released(&mut self, receipt: &Value) -> bool {
        settled(receipt) && !self.transport.owner_retained()
    }

    /// Mark `operation_id` as the adapter's pending owner. The marker survives a
    /// refused recovery, so a failed resolution cannot re-open admission.
    fn retain_pending(
        &mut self,
        operation_id: &str,
        pid: u32,
        process_creation_time: Option<&str>,
    ) {
        self.pending = Some(operation_id.to_string());
        self.pending_pid = Some(pid);
        self.pending_creation_time = process_creation_time.map(str::to_string);
    }

    /// Drop the adapter's pending owner. Only a proven terminal release may
    /// reach this: every caller checks [`Self::owner_released`] first.
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
                if self.owner_released(&value) {
                    self.clear_pending();
                    return Ok(normalize(value));
                }
                return Err(retained_owner_error(operation_id, &value));
            }
            let inactive = value.get("phase").and_then(Value::as_str) == Some("completed")
                || value.get("active").and_then(Value::as_bool) != Some(true);
            if inactive && value.get("redirect_count").and_then(Value::as_u64) == Some(0) {
                // A dispatch that never redirected is released, not uncertain.
                let released = self.transport.release(operation_id)?;
                if self.owner_released(&released) {
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
        // The binding is re-checked here so a dispatch can never outrun the
        // identity gate, even if a caller skipped inspection.
        self.require_accepted_binding()?;
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
        self.retain_pending(
            operation_id,
            owner_pid,
            fields.get("process_creation_time").and_then(Value::as_str),
        );
        match self.transport.dispatch(mode, &Value::Object(params)) {
            Ok(value) => Ok(value),
            Err(error) => {
                if !self.transport.operation_known(operation_id) && !self.transport.owner_retained()
                {
                    // A proven pre-dispatch refusal keeps no ownership. It only
                    // clears this submission's own marker: a transport that
                    // still owns an earlier operation keeps the adapter closed.
                    self.clear_pending();
                }
                Err(error)
            }
        }
    }

    /// Every fact `LiveAddAdapter.inspect` resolves, as the stored plan.
    pub fn inspect_plan(&mut self) -> Result<(Value, Inventory, NativeIndex), RuntimeError> {
        let game_version = self.require_accepted_binding()?;
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
            capture_read_only(&mut view, &layout, game_version)?
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
        if serial.to_string() != inventory.serial_counter || serial == 0 {
            return Err(rejected(
                "Serial changed or exceeds this executor ABI range",
            ));
        }
        // The live counter is a qword, the saved generation serial is not: the
        // insertion stamps this counter into record `+0x28` and advances it to
        // `serial + 1`, so both values must stay inside the save format's
        // representable domain. Refuse before dispatch rather than let a live
        // value that no save can hold reach the mutation.
        if serial >= SAVE_GENERATION_SERIAL_MAX {
            return Err(rejected("Serial cannot be advanced within the save format"));
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
        let game_version = self.require_accepted_binding()?;
        let module_base = self.transport.module_base()?;
        let creation_time = self.transport.creation_time()?;
        let layout = inventory_layout(&self.layout);
        let (inventory, index) = {
            let mut view = TransportInventoryView {
                transport: &mut self.transport,
                module_base,
                creation_time: creation_time.clone(),
            };
            capture_read_only(&mut view, &layout, game_version)?
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
    ///
    /// It reports success only together with a proven terminal owner. A receipt
    /// that settles while the transport still holds an allocation or a debug
    /// session is refused: the operation stays the adapter's pending owner, so
    /// the failure cannot admit a second dispatch.
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
        // The operation owns the adapter until the proof exists, not until a
        // verification attempt happens to return.
        self.retain_pending(operation_id, pid, process_creation_time);
        let receipt = self.transport.status(operation_id)?;
        if self.owner_released(&receipt) {
            self.clear_pending();
            return Ok(normalize(receipt));
        }
        // Verification only: the transport re-reads the target and settles the
        // receipt when it can prove the outcome. It never dispatches again.
        let released = self.transport.release(operation_id)?;
        if self.owner_released(&released) {
            self.clear_pending();
            return Ok(normalize(released));
        }
        // Business verification is useful evidence, but it cannot recreate or
        // release the old allocation/debugger/thread owner, and a settled
        // receipt that outlives its owner is not proof either.
        Err(retained_owner_error(operation_id, &released))
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

    /// The display version this executor's binding was accepted with.
    fn display_version(&self) -> &str {
        &self.display_version
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
            .get("breakpoint_count")
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
        // The executor is pinned to the PC v2.01 manager-object ABI; the
        // version gate and the dispatch-signature read refuse anything else
        // before this layout is used.
        inventory_global_mode: Some(
            crate::mutation::inventory::INVENTORY_GLOBAL_MODE_MANAGER_OBJECT,
        ),
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
        DebugEvent, DebugSession, RuntimeOwnerSession, RuntimeOwnerSnapshot, WindowsDebugSession,
        EXCEPTION_BREAKPOINT, EXCEPTION_SINGLE_STEP,
    };

    /// How long the dispatch loop waits for the accepted idle window.
    ///
    /// This is real monotonic time before the redirect, matching the shipped
    /// `wait` of "15 s at a 50 ms poll" in spirit: an attach burst of thread and
    /// DLL events must not consume the observation period.
    const DISPATCH_IDLE_DEADLINE_MS: u64 = 10_000;
    /// Hard safety cap on debug events for one dispatch.
    ///
    /// A runaway event stream must not keep the debugger attached forever, but
    /// this is a distinct condition with its own failure reason: an attach burst
    /// is not an idle miss.
    const DISPATCH_EVENT_SAFETY_MAX: u64 = 10_000;
    /// Rounds allowed *after* the redirect, where the target already sits inside
    /// the shim. The round bound survives only here, so the unsafe window is
    /// never longer than the shipped 100-round bound it replaces.
    const DISPATCH_POST_REDIRECT_ROUNDS: u64 = 100;

    /// The bounded window one dispatch may use before and after the redirect.
    #[derive(Debug, Clone, Copy)]
    struct DispatchWindow {
        /// Pre-redirect acceptance deadline, measured with `Instant`.
        idle_deadline: std::time::Duration,
        /// Hard cap on loop iterations (one debug event or one wait timeout).
        event_safety_max: u64,
        /// Iterations allowed after the redirect, before acknowledgement.
        post_redirect_rounds: u64,
    }

    impl DispatchWindow {
        /// The shipped window: 10 s of idle observation before the redirect,
        /// then the unchanged 100-round post-redirect bound.
        const PRODUCT: Self = Self {
            idle_deadline: std::time::Duration::from_millis(DISPATCH_IDLE_DEADLINE_MS),
            event_safety_max: DISPATCH_EVENT_SAFETY_MAX,
            post_redirect_rounds: DISPATCH_POST_REDIRECT_ROUNDS,
        };
    }

    /// Why the dispatch loop stopped.
    const STOP_ACCEPTED: &str = "accepted";
    const STOP_IDLE_DEADLINE: &str = "idle_deadline";
    const STOP_EVENT_SAFETY_BOUND: &str = "event_safety_bound";
    const STOP_POST_REDIRECT_BOUND: &str = "post_redirect_bound";
    const STOP_EXIT_PROCESS: &str = "exit_process";
    const STOP_OWNERSHIP_FAILURE: &str = "ownership_failure";
    const STOP_ERROR: &str = "error";

    /// Event-kind and acceptance counters for one dispatch attempt.
    ///
    /// They exist because a settled receipt alone cannot separate "the entry was
    /// never executed" from "the entry was executed and the ownership recheck
    /// rejected it": the rejected path is deliberately silent.
    #[derive(Debug, Clone, Copy, Default)]
    struct DispatchDiagnostics {
        rounds: u64,
        wait_timeouts: u64,
        create_process: u64,
        create_thread: u64,
        load_dll: u64,
        exit_thread: u64,
        exit_process: u64,
        breakpoint_exceptions: u64,
        single_steps: u64,
        other_exceptions: u64,
        other_events: u64,
        entry_hits: u64,
        entry_hits_accepted: u64,
        entry_ownership_rejects: u64,
        entry_ownership_reject_manager: u64,
        entry_ownership_reject_data: u64,
        acknowledgement_hits: u64,
        threads_armed: u64,
        stop_reason: &'static str,
    }

    /// The receipt shape of one attempt's diagnostics.
    fn diagnostics_json(diagnostics: &DispatchDiagnostics, elapsed: std::time::Duration) -> Value {
        json!({
            "rounds": diagnostics.rounds,
            "wait_timeouts": diagnostics.wait_timeouts,
            "create_process": diagnostics.create_process,
            "create_thread": diagnostics.create_thread,
            "load_dll": diagnostics.load_dll,
            "exit_thread": diagnostics.exit_thread,
            "exit_process": diagnostics.exit_process,
            "breakpoint_exceptions": diagnostics.breakpoint_exceptions,
            "single_steps": diagnostics.single_steps,
            "other_exceptions": diagnostics.other_exceptions,
            "other_events": diagnostics.other_events,
            "entry_hits": diagnostics.entry_hits,
            "entry_hits_accepted": diagnostics.entry_hits_accepted,
            "entry_ownership_rejects": diagnostics.entry_ownership_rejects,
            "entry_ownership_reject_manager": diagnostics.entry_ownership_reject_manager,
            "entry_ownership_reject_data": diagnostics.entry_ownership_reject_data,
            "acknowledgement_hits": diagnostics.acknowledgement_hits,
            "threads_armed": diagnostics.threads_armed,
            "elapsed_ms": elapsed.as_millis().min(u128::from(u64::MAX)) as u64,
            "stop_reason": diagnostics.stop_reason,
        })
    }

    fn thread_cleanup_json(snapshot: &RuntimeOwnerSnapshot) -> Value {
        let mut threads = serde_json::Map::new();
        for thread in &snapshot.threads {
            threads.insert(
                format!("{}:{}", thread.tid, thread.instance),
                json!({
                    "tid": thread.tid,
                    "instance": thread.instance,
                    "handle": thread.handle,
                    "handle_provenance": thread.handle_provenance,
                    "run_state": thread.run_state,
                    "cleanup_state": thread.cleanup_state,
                    "error": thread.error,
                }),
            );
        }
        Value::Object(threads)
    }

    fn thread_cleanup_complete(snapshot: &RuntimeOwnerSnapshot) -> bool {
        snapshot
            .threads
            .iter()
            .all(|thread| COMPLETED_CLEANUP_STATES.contains(&thread.cleanup_state))
    }

    /// Establish one debugger-owned stop barrier for cleanup. Every event that
    /// is not the barrier is continued according to its actual ownership; a
    /// foreign exception is never swallowed as handled.
    fn cleanup_barrier<S: RuntimeOwnerSession>(
        session: &mut S,
        diagnostics: &mut DispatchDiagnostics,
    ) -> Result<DebugEvent, RuntimeError> {
        session.begin_cleanup_barrier()?;
        for _ in 0..DISPATCH_POST_REDIRECT_ROUNDS {
            let Some(event) = session.wait(100)? else {
                continue;
            };
            if event.is_create_thread() || event.is_create_process() {
                if event.is_create_process() {
                    diagnostics.create_process += 1;
                } else {
                    diagnostics.create_thread += 1;
                }
                if let Some(handle) = event.thread_handle {
                    session.adopt_thread(event.tid, handle)?;
                }
                session.resume(&event, true)?;
                continue;
            }
            if event.is_exit_thread() {
                diagnostics.exit_thread += 1;
                session.resume(&event, true)?;
                continue;
            }
            if event.is_exit_process() {
                diagnostics.exit_process += 1;
                session.resume(&event, true)?;
                return Err(dispatch_error("Process exited before cleanup barrier"));
            }
            if event.is_load_dll() || !event.is_exception() {
                session.resume(&event, true)?;
                continue;
            }
            let code = event.exception_code.unwrap_or_default();
            if code == EXCEPTION_BREAKPOINT {
                diagnostics.breakpoint_exceptions += 1;
                return Ok(event);
            }
            if code == EXCEPTION_SINGLE_STEP {
                diagnostics.single_steps += 1;
                let mut context = session.context(event.tid)?;
                let owned = context.dr6 & 3 != 0;
                if owned {
                    context.dr6 &= !3;
                    context.eflags |= 0x10000;
                    session.set_context(event.tid, &context)?;
                } else {
                    diagnostics.other_exceptions += 1;
                }
                session.resume(&event, owned)?;
                continue;
            }
            diagnostics.other_exceptions += 1;
            session.resume(&event, false)?;
        }
        Err(dispatch_error(
            "Debugger cleanup could not establish a stopped-event barrier",
        ))
    }

    /// The real transport: one `WindowsDebug` session per accepted dispatch.
    pub struct NativeDebugTransport {
        pid: u32,
        layout: LiveAddLayout,
        module_name: String,
        store: ReceiptStore,
        /// The allocation an uncertain redirect still owns.
        retained: Option<u64>,
        /// The exact debugger/process owner for unresolved cleanup. A new
        /// process view cannot recreate its pending event or adopted handles.
        retained_session: Option<WindowsDebugSession>,
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
                retained_session: None,
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
            layout: LiveAddLayout,
            receipt: &mut Value,
            session: &mut WindowsDebugSession,
        ) -> Result<bool, RuntimeError> {
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
                    record_business_outcome(receipt, "completed", true);
                    return Ok(true);
                }
            }
            if let (Some(slot), Some(planned)) = (slot, planned) {
                let destination =
                    session.read(container + slot * layout.record_size as u64, RECORD_SIZE)?;
                if destination[0] == 0 && destination[1] == 0 && serial == planned {
                    // The destination is still empty and the serial did not
                    // advance: the dispatch provably did not reach insertion.
                    record_business_outcome(receipt, "rejected", false);
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }

    fn record_business_outcome(receipt: &mut Value, phase: &str, completed: bool) {
        if let Some(object) = receipt.as_object_mut() {
            object.insert("phase".to_string(), json!(phase));
            object.insert("active".to_string(), json!(false));
            object.insert(
                "business_outcome".to_string(),
                json!(if completed { "committed" } else { "rejected" }),
            );
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

    fn read_session_u64<S: DebugSession>(
        session: &mut S,
        address: u64,
    ) -> Result<u64, RuntimeError> {
        let raw = session.read(address, 8)?;
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&raw);
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_session_u32<S: DebugSession>(
        session: &mut S,
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
                "busy": self.retained.is_some()
                    || self.retained_session.is_some()
                    || self.store.unresolved_owner()?.is_some(),
            }))
        }

        /// The exact attached executable, hashed from the image path the target
        /// itself reports. Read-only file access: no target memory is read and
        /// no debugger is opened, so a candidate binding can refuse before any
        /// read or dispatch.
        fn executable_sha256(&mut self) -> Result<Option<String>, RuntimeError> {
            let executable = crate::platform::query_image_path(self.pid)?;
            Ok(Some(sha256_hex(&read_bytes(Path::new(&executable))?)))
        }

        fn dispatch(&mut self, mode: DispatchMode, params: &Value) -> Result<Value, RuntimeError> {
            let operation_id = params
                .get("operation_id")
                .and_then(Value::as_str)
                .ok_or_else(|| rejected("Native admission needs an operation identity"))?
                .to_string();
            if self.store.exists(&operation_id)
                || self.retained.is_some()
                || self.retained_session.is_some()
            {
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
                "business_outcome": "pending",
                "remote_execution": "not_started",
                "allocation_state": "not_allocated",
                "debugger_state": "not_attached",
                "thread_cleanup": {},
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
                DispatchWindow::PRODUCT,
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
                        self.retained_session = None;
                        return Ok(stored);
                    }
                    self.retained = receipt.get("allocation").and_then(Value::as_u64);
                    self.retained_session = Some(session);
                    Err(dispatch_error(
                        "Native dispatch cleanup is unresolved; owner retained, do not retry",
                    ))
                }
                Err(error) => {
                    let stored = self.store.read(&operation_id)?;
                    if settled(&stored) {
                        self.retained_session = None;
                        return Err(error);
                    }
                    self.retained = stored.get("allocation").and_then(Value::as_u64);
                    self.retained_session = Some(session);
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
            let mut session = match self.retained_session.take() {
                Some(session) => session,
                None => self.open()?,
            };
            let verified = Self::verify_outcome(self.layout, &mut value, &mut session);
            if !settled(&value) {
                self.retained_session = Some(session);
            }
            if verified? {
                self.store.save(&value)?;
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

        /// The two owners this binding can still hold: the allocation an
        /// unresolved redirect retained, and the exact debugger/process session
        /// retained for unresolved cleanup. The durable half of the same fact is
        /// `ReceiptStore::unresolved_owner`, which admission already consults.
        fn owner_retained(&mut self) -> bool {
            self.retained.is_some() || self.retained_session.is_some()
        }

        fn safe_to_shutdown(&mut self) -> bool {
            self.retained.is_none()
                && self.retained_session.is_none()
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
    fn run_dispatch<S: RuntimeOwnerSession>(
        session: &mut S,
        store: &ReceiptStore,
        layout: &LiveAddLayout,
        window: DispatchWindow,
        mode: DispatchMode,
        params: &Value,
        receipt: &mut Value,
    ) -> Result<(), RuntimeError> {
        let mut allocation: Option<u64> = None;
        let mut allocated_once = false;
        let mut redirected = false;
        let mut acknowledged = false;
        let mut failure: Option<RuntimeError> = None;
        let started = std::time::Instant::now();
        let mut diagnostics = DispatchDiagnostics::default();

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
            allocated_once = true;
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
            let mut rounds: u64 = 0;
            let mut post_redirect_rounds: u64 = 0;
            while chosen.is_none() {
                rounds += 1;
                diagnostics.rounds = rounds;
                if !redirected && started.elapsed() >= window.idle_deadline {
                    // Real monotonic time before the redirect: an attach burst of
                    // thread and DLL events must not consume the window.
                    diagnostics.stop_reason = STOP_IDLE_DEADLINE;
                    failure = Some(dispatch_error("No accepted idle dispatch before timeout"));
                    break;
                }
                if rounds > window.event_safety_max {
                    diagnostics.stop_reason = STOP_EVENT_SAFETY_BOUND;
                    failure = Some(dispatch_error(
                        "Native dispatch event safety bound reached before the idle window",
                    ));
                    break;
                }
                if redirected {
                    // Every iteration after the redirect counts, exactly like the
                    // shipped round bound, so the unsafe window is never longer.
                    post_redirect_rounds += 1;
                    if post_redirect_rounds > window.post_redirect_rounds {
                        diagnostics.stop_reason = STOP_POST_REDIRECT_BOUND;
                        failure = Some(dispatch_error(
                            "Native dispatch did not acknowledge before the post-redirect bound",
                        ));
                        break;
                    }
                }
                let Some(event) = session.wait(100)? else {
                    diagnostics.wait_timeouts += 1;
                    if rounds > 20 && !redirected {
                        // Nudge the mission thread once the idle window is late.
                        session.debug_break()?;
                    }
                    continue;
                };
                if event.is_exit_process() {
                    diagnostics.exit_process += 1;
                    diagnostics.stop_reason = STOP_EXIT_PROCESS;
                    allocation = None;
                    session.resume(&event, true)?;
                    return Err(dispatch_error("Game exited during native dispatch"));
                }
                if event.is_create_thread() || event.is_create_process() {
                    if event.is_create_process() {
                        diagnostics.create_process += 1;
                    } else {
                        diagnostics.create_thread += 1;
                    }
                    if let Some(handle) = event.thread_handle {
                        session.adopt_thread(event.tid, handle)?;
                        session.arm_thread(event.tid, entry, target)?;
                        diagnostics.threads_armed += 1;
                    }
                    session.resume(&event, true)?;
                    continue;
                }
                if event.is_load_dll() {
                    diagnostics.load_dll += 1;
                    session.resume(&event, true)?;
                    continue;
                }
                if event.is_exit_thread() {
                    diagnostics.exit_thread += 1;
                    session.resume(&event, true)?;
                    continue;
                }
                if !event.is_exception() {
                    diagnostics.other_events += 1;
                    session.resume(&event, true)?;
                    continue;
                }
                let code = event.exception_code.unwrap_or_default();
                if code == EXCEPTION_BREAKPOINT {
                    diagnostics.breakpoint_exceptions += 1;
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
                    diagnostics.other_exceptions += 1;
                    session.resume(&event, false)?;
                    continue;
                }
                diagnostics.single_steps += 1;
                let mut context = session.context(event.tid)?;
                if context.rip == entry && context.dr6 & 1 != 0 {
                    diagnostics.entry_hits += 1;
                    context.dr6 &= !1;
                    context.eflags |= 0x10000;
                    if !redirected {
                        // The accepted idle window: recheck every owner before
                        // the claim becomes durable.
                        let manager_now =
                            read_session_u64(session, base + layout.manager_pointer_rva)?;
                        let data_now = read_session_u64(session, manager_now)?;
                        if manager_now != manager || data_now != data {
                            diagnostics.entry_ownership_rejects += 1;
                            if manager_now != manager {
                                diagnostics.entry_ownership_reject_manager += 1;
                            }
                            if data_now != data {
                                diagnostics.entry_ownership_reject_data += 1;
                            }
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
                                diagnostics.stop_reason = STOP_OWNERSHIP_FAILURE;
                                failure =
                                    Some(dispatch_error("Serial or scheduler ownership changed"));
                                break;
                            }
                        }
                        diagnostics.entry_hits_accepted += 1;
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
                    diagnostics.acknowledgement_hits += 1;
                    diagnostics.stop_reason = STOP_ACCEPTED;
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
            Ok(())
        })();

        let mut cleanup_error: Option<RuntimeError> = None;
        if session.attached() {
            match cleanup_barrier(session, &mut diagnostics) {
                Ok(barrier) => match session.restore_threads() {
                    Ok(()) => {
                        if let Some(address) = allocation {
                            if !redirected || acknowledged {
                                if let Err(error) = session.free(address) {
                                    cleanup_error = Some(error);
                                } else {
                                    allocation = None;
                                }
                            }
                        }
                        if let Err(error) = session.resume(&barrier, true) {
                            cleanup_error.get_or_insert(error);
                        } else if let Err(error) = session.detach() {
                            cleanup_error.get_or_insert(error);
                        }
                    }
                    Err(error) => {
                        // Keep the stopped barrier pending, the debugger owner
                        // alive and the allocation retained. Continuing here
                        // would run a thread whose original debug registers are
                        // only partially restored.
                        cleanup_error = Some(error);
                    }
                },
                Err(error) => cleanup_error = Some(error),
            }
        } else if let Some(address) = allocation {
            // No debug state was installed. A pre-attach failure may retire its
            // allocation immediately without inventing debugger cleanup.
            if !redirected {
                if let Err(error) = session.free(address) {
                    cleanup_error = Some(error);
                } else {
                    allocation = None;
                }
            }
        }

        let error_text = match (&outcome, &failure, &cleanup_error) {
            (_, _, Some(error)) => Some(error.message()),
            (Err(error), _, None) => Some(error.message()),
            (Ok(()), Some(error), None) => Some(error.message()),
            (Ok(()), None, None) => None,
        };
        if diagnostics.stop_reason.is_empty() {
            // An error path that broke out of the loop without its own label.
            diagnostics.stop_reason = if outcome.is_err() {
                STOP_ERROR
            } else {
                STOP_ACCEPTED
            };
        }
        let elapsed = started.elapsed();
        let snapshot = session.runtime_owner_snapshot();
        let threads_clean = thread_cleanup_complete(&snapshot);
        let debugger_clean = matches!(snapshot.debugger_state, "detached" | "not_attached");
        let released = allocation.is_none() && threads_clean && debugger_clean;
        let business_outcome = if acknowledged && outcome.is_ok() && failure.is_none() {
            if mode == DispatchMode::Insert {
                "committed"
            } else {
                "completed"
            }
        } else if !redirected {
            "rejected"
        } else {
            "unknown"
        };
        let remote_execution = if acknowledged {
            "quiescent"
        } else if redirected {
            "unknown"
        } else {
            "not_started"
        };
        let allocation_state = if allocation.is_some() {
            "retained"
        } else if allocated_once {
            "freed"
        } else {
            "not_allocated"
        };
        let phase = if released && matches!(business_outcome, "committed" | "completed") {
            "completed"
        } else if released && business_outcome == "rejected" {
            "rejected"
        } else {
            "uncertain"
        };
        if let Some(object) = receipt.as_object_mut() {
            object.insert(
                "diagnostics".to_string(),
                diagnostics_json(&diagnostics, elapsed),
            );
            object.insert("phase".to_string(), json!(phase));
            object.insert("error".to_string(), json!(error_text));
            object.insert("active".to_string(), json!(false));
            object.insert("released".to_string(), json!(released));
            object.insert(
                "breakpoint_count".to_string(),
                json!(if threads_clean { 0 } else { -1 }),
            );
            object.insert("business_outcome".to_string(), json!(business_outcome));
            object.insert("remote_execution".to_string(), json!(remote_execution));
            object.insert("allocation_state".to_string(), json!(allocation_state));
            object.insert("debugger_state".to_string(), json!(snapshot.debugger_state));
            object.insert("thread_cleanup".to_string(), thread_cleanup_json(&snapshot));
            object.insert(
                "allocation".to_string(),
                match allocation {
                    Some(address) => json!(address),
                    None => Value::Null,
                },
            );
        }
        store.save(receipt)?;
        if !released {
            return Err(dispatch_error(
                "Native dispatch cleanup is unresolved; retain ownership and do not retry",
            ));
        }
        match (outcome, failure) {
            (Err(error), _) => Err(error),
            (Ok(()), Some(error)) => Err(error),
            (Ok(()), None) => Ok(()),
        }
    }

    #[cfg(test)]
    #[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    mod dispatch_window_tests {
        use super::*;
        use crate::mutation::live_fakes::{
            ByteMemory, FIXTURE_BASE, FIXTURE_CREATION, FIXTURE_PID,
        };
        use crate::mutation::native_abi::PC_V202_LIVE_ADD_CANDIDATE;
        use crate::mutation::native_fakes::{FakeDebugSession, RuntimeOwnerFaults};
        use crate::mutation::win_session::{
            DebugEvent, OwnerThreadSnapshot, ThreadContext, EXCEPTION_SINGLE_STEP,
        };
        use std::path::PathBuf;
        use std::time::Duration;

        /// What the scripted debug stream delivers before the loop starts.
        #[derive(Debug, Clone, Copy)]
        enum Script {
            None,
            EntryOnly,
            BurstThenEntryThenAck(u32),
            ForeignThenEntryThenAck,
        }

        fn scratch(name: &str) -> PathBuf {
            let dir = std::env::temp_dir().join(format!(
                "nioh3-dispatch-window-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("scratch dir");
            dir
        }

        /// One synthetic image. `drift` makes the shim allocation land on the
        /// manager slot, so the ownership recheck reads a data pointer the shim
        /// write already changed.
        fn fake_session(drift: bool) -> FakeDebugSession {
            let layout = PC_V202_LIVE_ADD_CANDIDATE;
            let base = FIXTURE_BASE;
            let mut memory = ByteMemory::default();
            memory.write(base + layout.dispatch_rva, &layout.dispatch_signature);
            let allocate = base + 0x1_0000_0000;
            let (manager, data) = if drift {
                (allocate, base + 0x3_0000)
            } else {
                (base + 0x1_0000, base + 0x2_0000)
            };
            memory.write(base + layout.manager_pointer_rva, &manager.to_le_bytes());
            memory.write(manager, &data.to_le_bytes());
            FakeDebugSession::from_parts(FIXTURE_PID, memory, base, FIXTURE_CREATION)
        }

        fn thread_event(index: u32) -> DebugEvent {
            DebugEvent {
                code: 2,
                tid: if index < 2 { index + 1 } else { 1_000 + index },
                thread_handle: Some(0x1000 + u64::from(index)),
                ..DebugEvent::default()
            }
        }

        fn step_event(tid: u32) -> DebugEvent {
            DebugEvent {
                code: 1,
                tid,
                exception_code: Some(EXCEPTION_SINGLE_STEP),
                ..DebugEvent::default()
            }
        }

        fn run(
            name: &str,
            window: DispatchWindow,
            script: Script,
            drift: bool,
        ) -> (Result<(), RuntimeError>, Value) {
            let (outcome, receipt, _) =
                run_with_faults(name, window, script, drift, RuntimeOwnerFaults::default());
            (outcome, receipt)
        }

        fn run_with_faults(
            name: &str,
            window: DispatchWindow,
            script: Script,
            drift: bool,
            faults: RuntimeOwnerFaults,
        ) -> (Result<(), RuntimeError>, Value, FakeDebugSession) {
            let directory = scratch(name);
            let store = ReceiptStore::new(&directory).expect("receipt store");
            let layout = PC_V202_LIVE_ADD_CANDIDATE;
            let entry = FIXTURE_BASE + layout.dispatch_rva;
            let target = entry + 7;
            let mut session = fake_session(drift);
            session.owner.faults = faults;
            match script {
                Script::None => {}
                Script::EntryOnly => {
                    session.events.push_back(thread_event(0));
                    session.events.push_back(step_event(1));
                }
                Script::BurstThenEntryThenAck(count) => {
                    for index in 0..count {
                        session.events.push_back(thread_event(index));
                    }
                    session.events.push_back(step_event(1));
                    session.events.push_back(step_event(2));
                }
                Script::ForeignThenEntryThenAck => {
                    session.events.push_back(thread_event(0));
                    session.events.push_back(thread_event(1));
                    session.events.push_back(DebugEvent {
                        code: 1,
                        tid: 1,
                        exception_code: Some(0x4242_4242),
                        ..DebugEvent::default()
                    });
                    session.events.push_back(step_event(1));
                    session.events.push_back(step_event(2));
                }
            }
            session.contexts.insert(
                1,
                ThreadContext {
                    rip: entry,
                    dr6: 1,
                    ..ThreadContext::default()
                },
            );
            session.contexts.insert(
                2,
                ThreadContext {
                    rip: target,
                    dr6: 2,
                    ..ThreadContext::default()
                },
            );
            let params = json!({
                "operation_id": "test-dispatch-window",
                "process_creation_time": FIXTURE_CREATION.to_string(),
            });
            let mut receipt = json!({
                "operation_id": "test-dispatch-window",
                "pid": FIXTURE_PID,
                "process_creation_time": FIXTURE_CREATION.to_string(),
                "phase": "preparing",
                "active": true,
                "released": false,
                "redirect_count": 0,
                "breakpoint_count": -1,
                "mode": "noop",
            });
            let outcome = run_dispatch(
                &mut session,
                &store,
                &layout,
                window,
                DispatchMode::Noop,
                &params,
                &mut receipt,
            );
            (outcome, receipt, session)
        }

        fn diagnostic(receipt: &Value, key: &str) -> u64 {
            receipt["diagnostics"][key].as_u64().unwrap_or(u64::MAX)
        }

        /// The shipped 10 s window is wall-clock time: a burst of attach events
        /// must not consume it, which the old 100-round proxy did.
        #[test]
        fn an_attach_event_burst_does_not_consume_the_idle_deadline() {
            let (outcome, receipt) = run(
                "burst",
                DispatchWindow::PRODUCT,
                Script::BurstThenEntryThenAck(500),
                false,
            );
            assert!(outcome.is_ok(), "{outcome:?}");
            assert_eq!(receipt["phase"], "completed");
            assert_eq!(receipt["redirect_count"], 1);
            assert_eq!(diagnostic(&receipt, "create_thread"), 500);
            assert_eq!(diagnostic(&receipt, "entry_hits"), 1);
            assert_eq!(diagnostic(&receipt, "entry_hits_accepted"), 1);
            assert_eq!(diagnostic(&receipt, "acknowledgement_hits"), 1);
            assert_eq!(receipt["diagnostics"]["stop_reason"], "accepted");
            assert!(
                diagnostic(&receipt, "rounds") < DISPATCH_EVENT_SAFETY_MAX,
                "the burst must not reach the safety bound"
            );
        }

        /// One expired deadline rejects the attempt with the shipped idle-miss
        /// wording, releases everything and records why it stopped.
        #[test]
        fn the_idle_deadline_rejects_safely() {
            let window = DispatchWindow {
                idle_deadline: Duration::ZERO,
                event_safety_max: 10,
                post_redirect_rounds: DISPATCH_POST_REDIRECT_ROUNDS,
            };
            let (outcome, receipt) = run("deadline", window, Script::None, false);
            let error = outcome.expect_err("an expired deadline rejects the attempt");
            assert_eq!(error.message(), "No accepted idle dispatch before timeout");
            assert_eq!(receipt["phase"], "rejected");
            assert_eq!(receipt["redirect_count"], 0);
            assert_eq!(receipt["allocation"], Value::Null);
            assert_eq!(receipt["released"], true);
            assert_eq!(receipt["breakpoint_count"], 0);
            assert_eq!(receipt["diagnostics"]["stop_reason"], "idle_deadline");
            assert_eq!(diagnostic(&receipt, "rounds"), 1);
            assert_eq!(diagnostic(&receipt, "entry_hits"), 0);
        }

        /// An idle stream and a real entry hit rejected by the ownership recheck
        /// settle the same way, so only the diagnostics separate them.
        #[test]
        fn a_no_event_timeout_and_a_rejected_hit_are_distinguished() {
            let window = DispatchWindow {
                idle_deadline: Duration::from_secs(10),
                event_safety_max: 40,
                post_redirect_rounds: DISPATCH_POST_REDIRECT_ROUNDS,
            };
            let (no_event, no_event_receipt) = run("no-event", window, Script::None, false);
            assert!(no_event.is_err());
            assert_eq!(
                no_event_receipt["diagnostics"]["stop_reason"],
                "event_safety_bound"
            );
            assert_eq!(diagnostic(&no_event_receipt, "entry_hits"), 0);
            assert_eq!(diagnostic(&no_event_receipt, "entry_ownership_rejects"), 0);
            assert_eq!(diagnostic(&no_event_receipt, "wait_timeouts"), 40);

            let (rejected, rejected_receipt) = run("rejected-hit", window, Script::EntryOnly, true);
            assert!(rejected.is_err());
            assert_eq!(rejected_receipt["redirect_count"], 0);
            assert_eq!(diagnostic(&rejected_receipt, "entry_hits"), 1);
            assert_eq!(diagnostic(&rejected_receipt, "entry_hits_accepted"), 0);
            assert_eq!(diagnostic(&rejected_receipt, "entry_ownership_rejects"), 1);
            assert_eq!(
                diagnostic(&rejected_receipt, "entry_ownership_reject_data"),
                1
            );
            assert_eq!(
                diagnostic(&rejected_receipt, "entry_ownership_reject_manager"),
                0
            );
        }

        /// The post-redirect bound is unchanged: a redirect that never
        /// acknowledges settles uncertain and retains its allocation.
        #[test]
        fn the_post_redirect_bound_still_guards_the_unsafe_window() {
            let (outcome, receipt) = run(
                "post-redirect",
                DispatchWindow::PRODUCT,
                Script::EntryOnly,
                false,
            );
            let error = outcome.expect_err("an unacknowledged redirect is uncertain");
            assert!(
                error.message().contains("cleanup is unresolved"),
                "{error:?}"
            );
            assert_eq!(receipt["phase"], "uncertain");
            assert_eq!(receipt["redirect_count"], 1);
            assert_eq!(receipt["released"], false);
            assert_eq!(receipt["breakpoint_count"], 0);
            assert_eq!(receipt["allocation_state"], "retained");
            assert_eq!(receipt["debugger_state"], "detached");
            assert_ne!(receipt["allocation"], Value::Null);
            assert_eq!(receipt["diagnostics"]["stop_reason"], "post_redirect_bound");
            let rounds = diagnostic(&receipt, "rounds");
            assert!(
                (DISPATCH_POST_REDIRECT_ROUNDS..=DISPATCH_POST_REDIRECT_ROUNDS + 3)
                    .contains(&rounds),
                "the post-redirect allowance stays at the shipped bound: {rounds}"
            );
        }

        #[test]
        fn partial_restore_retains_every_unresolved_owner_axis() {
            let (outcome, receipt, session) = run_with_faults(
                "partial-restore",
                DispatchWindow::PRODUCT,
                Script::BurstThenEntryThenAck(2),
                false,
                RuntimeOwnerFaults {
                    restore_tid: Some(2),
                    detach: false,
                },
            );
            let error = outcome.expect_err("partial restore must fail closed");
            assert!(error.message().contains("cleanup is unresolved"));
            assert_eq!(receipt["phase"], "uncertain");
            assert_eq!(receipt["business_outcome"], "completed");
            assert_eq!(receipt["remote_execution"], "quiescent");
            assert_eq!(receipt["allocation_state"], "retained");
            assert_eq!(receipt["debugger_state"], "attached");
            assert_eq!(receipt["released"], false);
            assert_eq!(receipt["breakpoint_count"], -1);
            assert_eq!(
                receipt["thread_cleanup"]["1:1"]["cleanup_state"],
                "original_restored"
            );
            assert_eq!(
                receipt["thread_cleanup"]["2:2"]["cleanup_state"],
                "restore_failed"
            );
            assert!(session.freed.is_empty());
            assert!(session.owner.current_event.is_some());
            assert_eq!(session.owner.detach_attempts, 0);
        }

        #[test]
        fn detach_failure_does_not_erase_successful_business_or_cleanup_facts() {
            let (outcome, receipt, session) = run_with_faults(
                "detach-failure",
                DispatchWindow::PRODUCT,
                Script::BurstThenEntryThenAck(2),
                false,
                RuntimeOwnerFaults {
                    restore_tid: None,
                    detach: true,
                },
            );
            let error = outcome.expect_err("detach failure must retain debugger ownership");
            assert!(error.message().contains("cleanup is unresolved"));
            assert_eq!(receipt["phase"], "uncertain");
            assert_eq!(receipt["business_outcome"], "completed");
            assert_eq!(receipt["remote_execution"], "quiescent");
            assert_eq!(receipt["allocation_state"], "freed");
            assert_eq!(receipt["debugger_state"], "detach_failed");
            assert_eq!(receipt["released"], false);
            assert_eq!(receipt["breakpoint_count"], 0);
            assert_eq!(
                receipt["thread_cleanup"]["1:1"]["cleanup_state"],
                "original_restored"
            );
            assert_eq!(
                receipt["thread_cleanup"]["2:2"]["cleanup_state"],
                "original_restored"
            );
            assert_eq!(session.freed.len(), 1);
            assert!(session.owner.current_event.is_none());
            assert_eq!(session.owner.detach_attempts, 1);
            assert!(session.owner.attached);
        }

        #[test]
        fn late_foreign_exception_is_continued_as_not_handled() {
            let (outcome, receipt, session) = run_with_faults(
                "foreign-exception",
                DispatchWindow::PRODUCT,
                Script::ForeignThenEntryThenAck,
                false,
                RuntimeOwnerFaults::default(),
            );
            assert!(outcome.is_ok(), "{outcome:?}");
            assert_eq!(receipt["business_outcome"], "completed");
            assert_eq!(receipt["released"], true);
            assert!(session
                .owner
                .resumes
                .iter()
                .any(|(event, handled)| { event.exception_code == Some(0x4242_4242) && !handled }));
        }

        /// The retired record's identity in the injected owner report.
        const RETIRED_TID: u32 = 77;
        const RETIRED_INSTANCE: u64 = 9;
        const RETIRED_HANDLE: u64 = 0x5000;

        /// A `FakeDebugSession` whose owner report carries one extra retired
        /// record with a chosen cleanup state.
        ///
        /// The offline owner maps every retired record to `exited`
        /// (`native_fakes.rs`), so nothing in the suite could put the production
        /// `handle_reused` string into a receipt. The real binding emits it from
        /// `retire_thread_at(index, "handle_reused")` whenever a create event
        /// adopts a numeric value whose earlier record Windows already closed,
        /// terminated, or handed on (`win_session.rs`). This wrapper is that
        /// report, driven through the shipped `run_dispatch` receipt path.
        struct RetiredRecordSession {
            inner: FakeDebugSession,
            state: &'static str,
        }

        impl DebugSession for RetiredRecordSession {
            fn pid(&self) -> u32 {
                self.inner.pid()
            }

            fn creation_time(&mut self) -> Result<String, RuntimeError> {
                self.inner.creation_time()
            }

            fn module_base(&mut self) -> Result<u64, RuntimeError> {
                self.inner.module_base()
            }

            fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
                self.inner.read(address, size)
            }

            fn write(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError> {
                self.inner.write(address, data)
            }

            fn allocate(&mut self, size: usize) -> Result<u64, RuntimeError> {
                self.inner.allocate(size)
            }

            fn free(&mut self, address: u64) -> Result<(), RuntimeError> {
                self.inner.free(address)
            }

            fn flush_instruction_cache(
                &mut self,
                address: u64,
                size: usize,
            ) -> Result<(), RuntimeError> {
                self.inner.flush_instruction_cache(address, size)
            }

            fn attach(&mut self) -> Result<(), RuntimeError> {
                self.inner.attach()
            }

            fn attached(&self) -> bool {
                self.inner.attached()
            }

            fn detach(&mut self) -> Result<(), RuntimeError> {
                self.inner.detach()
            }

            fn wait(&mut self, milliseconds: u32) -> Result<Option<DebugEvent>, RuntimeError> {
                self.inner.wait(milliseconds)
            }

            fn resume(&mut self, event: &DebugEvent, handled: bool) -> Result<(), RuntimeError> {
                self.inner.resume(event, handled)
            }

            fn context(&mut self, tid: u32) -> Result<ThreadContext, RuntimeError> {
                self.inner.context(tid)
            }

            fn set_context(
                &mut self,
                tid: u32,
                context: &ThreadContext,
            ) -> Result<(), RuntimeError> {
                self.inner.set_context(tid, context)
            }

            fn adopt_thread(&mut self, tid: u32, handle: u64) -> Result<(), RuntimeError> {
                self.inner.adopt_thread(tid, handle)
            }

            fn arm_thread(
                &mut self,
                tid: u32,
                entry: u64,
                acknowledgement: u64,
            ) -> Result<(), RuntimeError> {
                self.inner.arm_thread(tid, entry, acknowledgement)
            }

            fn restore_threads(&mut self) -> Result<(), RuntimeError> {
                self.inner.restore_threads()
            }

            fn all_threads_exited(&mut self) -> Result<bool, RuntimeError> {
                self.inner.all_threads_exited()
            }

            fn thread_signalled(&mut self, tid: u32) -> Result<bool, RuntimeError> {
                self.inner.thread_signalled(tid)
            }

            fn debug_break(&mut self) -> Result<(), RuntimeError> {
                self.inner.debug_break()
            }
        }

        impl RuntimeOwnerSession for RetiredRecordSession {
            fn begin_cleanup_barrier(&mut self) -> Result<(), RuntimeError> {
                self.inner.begin_cleanup_barrier()
            }

            fn runtime_owner_snapshot(&self) -> RuntimeOwnerSnapshot {
                let mut snapshot = self.inner.runtime_owner_snapshot();
                // Only `cleanup_state` is authoritative; `run_state` is
                // diagnostic and mirrors the production taxonomy: a retired
                // record is `exited`, a retained or unknown one is stopped.
                let run_state = match self.state {
                    "handle_reused" | "unknown" => "exited",
                    _ => "stopped",
                };
                snapshot.threads.push(OwnerThreadSnapshot {
                    tid: RETIRED_TID,
                    instance: RETIRED_INSTANCE,
                    handle: RETIRED_HANDLE,
                    handle_provenance: "create_thread_event",
                    run_state,
                    cleanup_state: self.state,
                    error: None,
                });
                snapshot
            }
        }

        /// Drive the shipped dispatch loop over a session whose owner report
        /// carries one retired record with `state`
        /// (`win_session.rs` merges retired records into the same report).
        fn run_with_retired_record(
            name: &str,
            state: &'static str,
        ) -> (Result<(), RuntimeError>, Value) {
            let directory = scratch(name);
            let store = ReceiptStore::new(&directory).expect("receipt store");
            let layout = PC_V202_LIVE_ADD_CANDIDATE;
            let entry = FIXTURE_BASE + layout.dispatch_rva;
            let target = entry + 7;
            let mut inner = fake_session(false);
            inner.events.push_back(thread_event(0));
            inner.events.push_back(thread_event(1));
            inner.events.push_back(step_event(1));
            inner.events.push_back(step_event(2));
            inner.contexts.insert(
                1,
                ThreadContext {
                    rip: entry,
                    dr6: 1,
                    ..ThreadContext::default()
                },
            );
            inner.contexts.insert(
                2,
                ThreadContext {
                    rip: target,
                    dr6: 2,
                    ..ThreadContext::default()
                },
            );
            let mut session = RetiredRecordSession { inner, state };
            let params = json!({
                "operation_id": "test-dispatch-window",
                "process_creation_time": FIXTURE_CREATION.to_string(),
            });
            let mut receipt = json!({
                "operation_id": "test-dispatch-window",
                "pid": FIXTURE_PID,
                "process_creation_time": FIXTURE_CREATION.to_string(),
                "phase": "preparing",
                "active": true,
                "released": false,
                "redirect_count": 0,
                "breakpoint_count": -1,
                "mode": "noop",
            });
            let outcome = run_dispatch(
                &mut session,
                &store,
                &layout,
                DispatchWindow::PRODUCT,
                DispatchMode::Noop,
                &params,
                &mut receipt,
            );
            (outcome, receipt)
        }

        /// R2-A. A create-event record Windows closed, terminated, or handed to
        /// another thread is retired as `handle_reused`: the owner is gone, so
        /// the shipped loop must settle it and release. While the settled
        /// predicate rejected that state the exact dispatch R2 enables could
        /// never release its owner, and R1 then refused every recovery path on
        /// the unsettled record for good.
        #[test]
        fn a_handle_reused_retirement_settles_and_releases() {
            let (outcome, receipt) = run_with_retired_record("handle-reused", "handle_reused");
            assert!(outcome.is_ok(), "{outcome:?}");
            assert_eq!(receipt["phase"], "completed");
            assert_eq!(receipt["released"], true);
            assert_eq!(receipt["active"], false);
            assert_eq!(receipt["breakpoint_count"], 0);
            assert!(settled(&receipt), "the shipped predicate accepts it");
            assert_eq!(
                receipt["thread_cleanup"]["77:9"]["cleanup_state"], "handle_reused",
                "the reuse evidence survives into the receipt"
            );
            assert_eq!(
                receipt["thread_cleanup"]["77:9"]["handle"], RETIRED_HANDLE,
                "the closed value is kept as provenance"
            );
        }

        /// The same retirement path stays fail-closed for every state that is
        /// not a completed retirement: still-armed (retained), restore-failed
        /// and genuinely unknown records keep the owner.
        #[test]
        fn a_non_completed_retirement_never_settles() {
            for state in ["armed", "restore_failed", "unknown"] {
                let (outcome, receipt) =
                    run_with_retired_record(&format!("retained-{state}"), state);
                let error = outcome.expect_err("an unresolved record must fail closed");
                assert!(
                    error.message().contains("cleanup is unresolved"),
                    "{state}: {error}"
                );
                assert_eq!(receipt["phase"], "uncertain", "{state}");
                assert_eq!(receipt["released"], false, "{state}");
                assert_eq!(receipt["breakpoint_count"], -1, "{state}");
                assert!(!settled(&receipt), "{state} is not a settled receipt");
                assert_eq!(
                    receipt["thread_cleanup"]["77:9"]["cleanup_state"], state,
                    "{state}"
                );
            }
        }
    }
}

#[cfg(windows)]
pub use windows_transport::NativeDebugTransport;
