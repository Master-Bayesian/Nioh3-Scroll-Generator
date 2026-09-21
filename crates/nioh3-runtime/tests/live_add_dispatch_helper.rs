//! Real Windows acceptance for one future preview settlement.
//!
//! The injected transports prove the preview state machine; the scripted
//! `run_dispatch` tests prove the settlement predicates. This test drives the
//! **real** path end to end against the owned disposable helper the other
//! Windows tests already use: `NativeDebugTransport` opens a real debug session,
//! the executor redirects the helper's own thread into its shim, the shim calls
//! the helper's builder stand-in, and the mismatch is settled as
//! `rejected_after_preview` with a provably unchanged inventory and terminal
//! cleanup. Read-only recovery must then run the builder zero times.
//!
//! It never touches Nioh 3, a game, a save, or a debugger session on any other
//! process. The helper is the only target, and it is created by this test.
#![cfg(all(feature = "test-helper", windows))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use nioh3_runtime::mutation::descriptor::new_assembly_record;
use nioh3_runtime::mutation::inventory::{
    capture_index, capture_inventory, capture_read_only, InventoryLayout, InventoryProcess,
};
use nioh3_runtime::mutation::native_abi::{PC_V201_LIVE_ADD, SCROLL_RECORD_SIZE};
use nioh3_runtime::mutation::native_executor::{
    settled, NativeDebugTransport, NativeLiveAddExecutor,
};
use nioh3_runtime::mutation::{
    preview_rejection_complete, LiveAddExecutor, INVENTORY_GLOBAL_MODE_MANAGER_OBJECT,
    LIVE_ADD_DISPLAY_VERSION,
};
use nioh3_runtime::mutation::win_session::{RemoteSession, WindowsRemoteSession};
use nioh3_runtime::RuntimeError;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

/// Real debug sessions are inherited-handle lifecycles: keep them sequential in
/// this integration binary so one case cannot invalidate another case's pipes.
static LIVE_PREVIEW_LOCK: Mutex<()> = Mutex::new(());

fn lock_live_preview() -> std::sync::MutexGuard<'static, ()> {
    LIVE_PREVIEW_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The label the accepted PC v2.01 binding carries.
const PRODUCT_VERSION: &str = LIVE_ADD_DISPLAY_VERSION;

fn helper_image_name() -> String {
    Path::new(env!("CARGO_BIN_EXE_runtime_mutation_helper"))
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// One durable state root per case, on the task's build volume when the caller
/// points `NIOH3_PREVIEW_HELPER_STATE` at it.
fn state_base() -> PathBuf {
    std::env::var_os("NIOH3_PREVIEW_HELPER_STATE")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The reviewed installation record: the record the preview must reproduce.
fn reviewed_record() -> Vec<u8> {
    let mut raw = vec![0u8; SCROLL_RECORD_SIZE];
    raw[0] = 0x82;
    raw[1] = 0x1E;
    raw[0x18..0x1C].copy_from_slice(&0x0280_0002u32.to_le_bytes());
    raw[0x20..0x24].copy_from_slice(&0x0BAD_F00Du32.to_le_bytes());
    raw[0x30] = 4;
    raw[0x31] = 4;
    new_assembly_record(&raw).expect("reviewed assembly record")
}

/// What the builder stand-in writes: the reviewed record, with the preview
/// serial sentinel, optionally differing in the reviewed byte window.
fn builder_source(reviewed: &[u8], mismatch: bool) -> Vec<u8> {
    let mut source = reviewed.to_vec();
    source[0x28..0x30].copy_from_slice(&[0xFFu8; 8]);
    if mismatch {
        source[0x20] ^= 0xFF;
    }
    source
}

struct Helper {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    pid: u32,
    module_base: u64,
    state_root: PathBuf,
}

struct LiveTarget {
    builder_hex: String,
    creation: String,
    data: u64,
}

impl Helper {
    fn spawn(name: &str) -> Result<Self, RuntimeError> {
        let path = env!("CARGO_BIN_EXE_runtime_mutation_helper");
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|error| RuntimeError::Io {
                path: path.to_string(),
                detail: error.to_string(),
            })?;
        let stdin = child.stdin.take().ok_or(RuntimeError::SessionNotOpen)?;
        let stdout = BufReader::new(child.stdout.take().ok_or(RuntimeError::SessionNotOpen)?);
        let state_root = state_base().join(format!("nioh3-preview-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&state_root);
        std::fs::create_dir_all(&state_root).map_err(|error| RuntimeError::Io {
            path: state_root.display().to_string(),
            detail: error.to_string(),
        })?;
        let mut helper = Self {
            child,
            stdin,
            stdout,
            pid: 0,
            module_base: 0,
            state_root,
        };
        let fields: Vec<String> = helper
            .line()?
            .trim()
            .split('\t')
            .map(str::to_string)
            .collect();
        if fields.first().map(String::as_str) != Some("ready") {
            return Err(RuntimeError::SessionNotOpen);
        }
        helper.pid = fields
            .get(1)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        helper.module_base = fields
            .get(2)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        Ok(helper)
    }

    fn line(&mut self) -> Result<String, RuntimeError> {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .map_err(|error| RuntimeError::Io {
                path: "helper stdout".to_string(),
                detail: error.to_string(),
            })?;
        Ok(line)
    }

    fn command(&mut self, text: &str) -> Result<String, RuntimeError> {
        self.stdin
            .write_all(format!("{text}\n").as_bytes())
            .and_then(|()| self.stdin.flush())
            .map_err(|error| RuntimeError::Io {
                path: "helper stdin".to_string(),
                detail: error.to_string(),
            })?;
        self.line()
    }

    /// Present the accepted PC v2.01 live-add layout over this helper.
    fn live_add(&mut self, mode: &str, source: &[u8]) -> Result<LiveTarget, RuntimeError> {
        let line = self.command(&format!("live-add {mode} {}", hex(source)))?;
        let fields: Vec<&str> = line.trim().split('\t').collect();
        if fields.first() != Some(&"live-add") {
            return Err(RuntimeError::RuntimeBusy);
        }
        Ok(LiveTarget {
            builder_hex: fields.get(2).copied().unwrap_or_default().to_string(),
            creation: fields.get(4).copied().unwrap_or_default().to_string(),
            data: fields
                .get(5)
                .and_then(|value| u64::from_str_radix(value, 16).ok())
                .unwrap_or(0),
        })
    }

    /// The builder stand-in's own execution count, read from the helper.
    fn builder_calls(&mut self) -> Result<u64, RuntimeError> {
        let line = self.command("live-add-calls")?;
        let fields: Vec<&str> = line.trim().split('\t').collect();
        if fields.first() != Some(&"live-add-calls") {
            return Err(RuntimeError::RuntimeBusy);
        }
        Ok(fields
            .get(1)
            .and_then(|value| value.parse().ok())
            .unwrap_or(u64::MAX))
    }

    fn quit(&mut self) {
        let _ = self.stdin.write_all(b"quit\n");
        let _ = self.stdin.flush();
        // A failed dispatch can leave the helper frozen by a retained debug
        // session, so the exit is bounded and the owned child is killed if it
        // does not stop on its own.
        for _ in 0..40 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Every durable receipt this state root holds, newest content last.
fn receipts(directory: &Path) -> Vec<Value> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(directory) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                out.push(value);
            }
        }
    }
    out
}

fn preview_receipt(directory: &Path) -> Value {
    receipts(directory)
        .into_iter()
        .find(|value| value.get("mode").and_then(Value::as_str) == Some("preview"))
        .expect("one durable preview receipt")
}

fn operation_id(receipt: &Value) -> String {
    receipt
        .get("operation_id")
        .and_then(Value::as_str)
        .expect("receipt identity")
        .to_string()
}

/// Build the executor and plan that drive one real preview against the helper.
fn executor_for(
    helper: &Helper,
) -> Result<NativeLiveAddExecutor<NativeDebugTransport>, RuntimeError> {
    let transport = NativeDebugTransport::new(
        helper.pid,
        PC_V201_LIVE_ADD,
        &helper_image_name(),
        &helper.state_root,
    )?;
    Ok(NativeLiveAddExecutor::new(
        transport,
        PC_V201_LIVE_ADD,
        PRODUCT_VERSION,
    ))
}

fn plan(helper: &Helper, target: &LiveTarget) -> Value {
    json!({
        "pid": helper.pid,
        "process_creation_time": target.creation,
        "source_save_path": "",
        "candidate_id": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "parent_operation_id": "11111111-1111-4111-8111-111111111111",
        "builder_code_hex": target.builder_hex,
    })
}

/// A preview mismatch and the full terminal proof leave a durable rejected
/// child, and its read-only recovery reruns the builder zero times.
#[test]
fn a_real_preview_mismatch_settles_rejected_and_recovers_without_the_builder(
) -> Result<(), RuntimeError> {
    let _guard = lock_live_preview();
    let mut helper = Helper::spawn("mismatch")?;
    assert!(
        helper.module_base > 0,
        "the helper reports its own module base"
    );
    let reviewed = reviewed_record();
    let target = helper.live_add("mismatch", &builder_source(&reviewed, true))?;
    let mut executor = executor_for(&helper)?;
    let error = executor
        .preview(&plan(&helper, &target), &reviewed)
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(error.code(), "NATIVE_DISPATCH", "{error:?}");
    assert!(
        error.message().contains("differs from reviewed record"),
        "{error}"
    );

    let receipt = preview_receipt(&helper.state_root);
    let child = operation_id(&receipt);
    assert_eq!(
        receipt.get("phase").and_then(Value::as_str),
        Some("rejected_after_preview"),
        "{receipt}"
    );
    assert_eq!(
        receipt.get("business_outcome").and_then(Value::as_str),
        Some("rejected")
    );
    assert_eq!(
        receipt.get("settlement").and_then(Value::as_str),
        Some("rejected_after_preview")
    );
    assert_eq!(
        receipt.get("redirect_count").and_then(Value::as_u64),
        Some(1),
        "the rejection is after the dispatch, never before it"
    );
    assert!(settled(&receipt), "the terminal rejection releases its owner");
    assert!(preview_rejection_complete(&receipt), "{receipt}");

    // The inventory fingerprint pair is unchanged and names the same owner.
    let before = receipt.get("preview_before").expect("before fingerprint");
    let after = receipt.get("preview_after").expect("after fingerprint");
    for field in [
        "container_sha256",
        "native_index_digest",
        "index_node_count",
        "index_bucket_count",
        "capacity",
        "record_size",
        "serial_counter",
        "acquisition_order_counter",
        "manager",
        "data",
        "module_base",
    ] {
        assert_eq!(before.get(field), after.get(field), "{field}");
        assert!(before.get(field).is_some(), "{field} is recorded");
    }
    assert_eq!(
        receipt
            .get("preview_review")
            .and_then(|value| value.get("outcome"))
            .and_then(Value::as_str),
        Some("mismatch")
    );
    assert_eq!(
        receipt
            .get("preview_dispatch_proof")
            .and_then(|value| value.get("return_and_register_verified"))
            .and_then(Value::as_bool),
        Some(true)
    );

    // Read-only recovery settles the same child and never reruns the builder.
    let calls_after_dispatch = helper.builder_calls()?;
    assert_eq!(calls_after_dispatch, 1, "the shim called the builder once");
    let recovered = executor.recover(&child, helper.pid, Some(&target.creation))?;
    assert_eq!(
        recovered.get("phase").and_then(Value::as_str),
        Some("rejected_after_preview")
    );
    assert_eq!(
        recovered.get("operation_id").and_then(Value::as_str),
        Some(child.as_str())
    );
    assert_eq!(
        helper.builder_calls()?,
        calls_after_dispatch,
        "recovery re-ran the builder zero times"
    );
    helper.quit();
    Ok(())
}

/// The matched control: the same real path settles as a completed preview and
/// never presents the rejection.
#[test]
fn a_real_preview_match_settles_completed() -> Result<(), RuntimeError> {
    let _guard = lock_live_preview();
    let mut helper = Helper::spawn("matched")?;
    let reviewed = reviewed_record();
    let target = helper.live_add("matched", &builder_source(&reviewed, false))?;
    let mut executor = executor_for(&helper)?;
    let outcome = executor.preview(&plan(&helper, &target), &reviewed);
    eprintln!("PROBE matched preview outcome = {outcome:?}");
    let receipt = outcome?;
    assert_eq!(
        receipt.get("phase").and_then(Value::as_str),
        Some("completed"),
        "{receipt}"
    );
    assert_eq!(
        receipt.get("business_outcome").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(
        receipt
            .get("preview_review")
            .and_then(|value| value.get("outcome"))
            .and_then(Value::as_str),
        Some("matched")
    );
    assert!(settled(&receipt), "{receipt}");
    assert!(!preview_rejection_complete(&receipt));
    assert_eq!(helper.builder_calls()?, 1);
    helper.quit();
    Ok(())
}

/// A redirect without an acknowledgement stays blocked: no settlement, no
/// rejection, and no owner released.
#[test]
fn a_real_preview_without_acknowledgement_stays_blocked() -> Result<(), RuntimeError> {
    let _guard = lock_live_preview();
    let mut helper = Helper::spawn("noack")?;
    let reviewed = reviewed_record();
    let target = helper.live_add("noack", &builder_source(&reviewed, true))?;
    let mut executor = executor_for(&helper)?;
    let outcome = executor.preview(&plan(&helper, &target), &reviewed);
    assert!(outcome.is_err(), "{outcome:?}");

    let receipt = preview_receipt(&helper.state_root);
    assert_eq!(
        receipt.get("redirect_count").and_then(Value::as_u64),
        Some(1),
        "the redirect happened"
    );
    assert_eq!(
        receipt.get("business_outcome").and_then(Value::as_str),
        Some("unknown"),
        "{receipt}"
    );
    assert!(!settled(&receipt), "an unacknowledged preview never settles");
    assert!(!preview_rejection_complete(&receipt));
    helper.quit();
    Ok(())
}

/// The read-only process view the product's inventory capture needs.
struct ProbeView<'a> {
    session: &'a mut WindowsRemoteSession,
    pid: u32,
    module_base: u64,
    creation: String,
}

impl InventoryProcess for ProbeView<'_> {
    fn pid(&self) -> u32 {
        self.pid
    }

    fn module_base(&self) -> u64 {
        self.module_base
    }

    fn creation_time(&mut self) -> Result<String, RuntimeError> {
        Ok(self.creation.clone())
    }

    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
        self.session.read(address, size)
    }
}

/// Fixture validity, proved before any further debug work: the disposable helper
/// must present the exact bytes the product's preview baseline and read-only
/// inventory capture read, at the exact addresses and lengths, through the
/// product's own read path. No debugger is opened here, so this cannot hang.
#[test]
fn a_disposable_helper_presents_the_exact_preview_baseline_reads() -> Result<(), RuntimeError> {
    let _guard = lock_live_preview();
    let mut helper = Helper::spawn("baseline")?;
    let reviewed = reviewed_record();
    let target = helper.live_add("matched", &builder_source(&reviewed, false))?;
    assert!(target.data > 0, "the helper reports its data region");
    let layout = PC_V201_LIVE_ADD;
    let base = helper.module_base;
    let data = target.data;
    let mut session = WindowsRemoteSession::open(helper.pid)?;

    // The two pointer hops `resolve_inventory_pointers` makes, with values.
    let manager_slot = base + layout.manager_pointer_rva;
    let manager_value =
        u64::from_le_bytes(session.read(manager_slot, 8)?.try_into().unwrap_or_default());
    eprintln!(
        "PROBE manager slot {manager_slot:#x} = {manager_value:#x} (helper data {data:#x})"
    );
    match session.read(manager_value, 8) {
        Ok(raw) => eprintln!(
            "PROBE data via manager = {:#x}",
            u64::from_le_bytes(raw.try_into().unwrap_or_default())
        ),
        Err(error) => eprintln!("PROBE FAIL data via manager: {}", error.message()),
    }
    let index_header = data + layout.serial_index_offset;
    let header = session.read(index_header, 0x40)?;
    eprintln!(
        "PROBE index header {index_header:#x}: head={:#x} size={} buckets={:#x} mask={} bucket_count={}",
        u64::from_le_bytes(header[8..16].try_into().unwrap_or_default()),
        u64::from_le_bytes(header[16..24].try_into().unwrap_or_default()),
        u64::from_le_bytes(header[24..32].try_into().unwrap_or_default()),
        u64::from_le_bytes(header[0x30..0x38].try_into().unwrap_or_default()),
        u64::from_le_bytes(header[0x38..0x40].try_into().unwrap_or_default())
    );

    // Exactly what `capture_preview_fingerprint` reads, in its own order and
    // lengths, plus the two other sites the product inventory capture guards.
    let reads: [(&str, u64, usize); 9] = [
        (
            "preview container (capture_preview_fingerprint)",
            data + layout.container_offset,
            layout.capacity as usize * layout.record_size,
        ),
        (
            "preview serial counter",
            data + layout.serial_counter_offset,
            8,
        ),
        ("preview acquisition counter", data, 4),
        (
            "preview target builder (capture_preview_fingerprint)",
            base + layout.builder_rva,
            layout.builder_size as usize,
        ),
        (
            "dispatch entry signature",
            base + layout.dispatch_rva,
            layout.dispatch_signature.len(),
        ),
        ("manager pointer", base + layout.manager_pointer_rva, 8),
        ("insertion signature site", base + layout.insertion_rva, 0x20),
        (
            "container capacity cell",
            data + layout.container_offset + layout.capacity_offset,
            8,
        ),
        ("serial index", data + layout.serial_index_offset, 16),
    ];
    let mut failures: Vec<String> = Vec::new();
    for (name, address, size) in reads {
        match session.read(address, size) {
            Ok(bytes) => eprintln!("PROBE ok   {name}: {address:#x} len {size} -> {}", bytes.len()),
            Err(error) => {
                eprintln!("PROBE FAIL {name}: {address:#x} len {size} -> {}", error.message());
                failures.push(format!("{name}: {address:#x} len {size} -> {}", error.message()));
            }
        }
    }

    // The product's own read-only capture through the same read path.
    let inventory_layout = InventoryLayout {
        insertion_rva: layout.insertion_rva,
        manager_pointer_rva: layout.manager_pointer_rva,
        container_offset: layout.container_offset,
        capacity_offset: layout.capacity_offset,
        serial_index_offset: layout.serial_index_offset,
        capacity: layout.capacity,
        record_size: layout.record_size,
        serial_counter_offset: layout.serial_counter_offset,
        inventory_global_mode: Some(INVENTORY_GLOBAL_MODE_MANAGER_OBJECT),
    };
    let mut view = ProbeView {
        session: &mut session,
        pid: helper.pid,
        module_base: base,
        creation: target.creation.clone(),
    };
    // The product's own capture now also walks the real native serial index this
    // helper supplies; the same write is what the runtime proof will read before
    // and after.
    match capture_inventory(&mut view, &inventory_layout, PRODUCT_VERSION) {
        Ok(inventory) => eprintln!(
            "PROBE ok   capture_inventory: entries={} serial={} container={}",
            inventory.entries.len(),
            inventory.serial_counter,
            inventory.container_sha256
        ),
        Err(error) => {
            eprintln!("PROBE FAIL capture_inventory: {}", error.message());
            failures.push(format!("capture_inventory: {}", error.message()));
        }
    }
    match capture_index(&mut view, &inventory_layout, PRODUCT_VERSION) {
        Ok(index) => eprintln!(
            "PROBE ok   capture_index: nodes={} buckets={}",
            index.node_count, index.bucket_count
        ),
        Err(error) => {
            eprintln!("PROBE FAIL capture_index: {}", error.message());
            failures.push(format!("capture_index: {}", error.message()));
        }
    }
    let captured = capture_read_only(&mut view, &inventory_layout, PRODUCT_VERSION);
    match &captured {
        Ok((inventory, index)) => {
            let mut mapping: std::collections::BTreeMap<String, u32> =
                std::collections::BTreeMap::new();
            for entry in &index.entries {
                mapping.insert(entry.serial.clone(), entry.slot);
            }
            let canonical = mapping
                .iter()
                .map(|(serial, slot)| format!("{serial}:{slot}"))
                .collect::<Vec<_>>()
                .join("\n");
            eprintln!(
                "PROBE ok   capture_read_only: entries={} serial={} container={} nodes={} buckets={} canonical={canonical:?}",
                inventory.entries.len(),
                inventory.serial_counter,
                inventory.container_sha256,
                index.node_count,
                index.bucket_count
            );
        }
        Err(error) => {
            eprintln!("PROBE FAIL capture_read_only: {}", error.message());
            failures.push(format!("capture_read_only: {}", error.message()));
        }
    }
    if let Ok((_, index)) = captured {
        assert_eq!(index.node_count, 2, "two native index nodes");
        assert_eq!(index.bucket_count, 1, "one FNV bucket");
    }
    helper.quit();
    assert!(failures.is_empty(), "fixture read failures: {failures:#?}");
    Ok(())
}
