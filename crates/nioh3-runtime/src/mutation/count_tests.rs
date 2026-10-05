//! Plan, execute, receipt and recovery tests for count edits.
//!
//! Every write is injected: the fake record window holds the bytes, so a test
//! can prove that a rejected or rejected-after-gate operation never reached
//! memory, and that an ambiguous outcome is never replayed.

// Tests assert on outcomes directly; the crate's production lints still keep
// `expect`, `unwrap` and `panic` out of non-test code.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::count::{
    canonical_json, checked_count, sha256_hex, stable_identity, CountEditor, CountState,
    RECORD_SIZE,
};
use super::fake::{FakeCountMemory, Faults};
use crate::error::RuntimeError;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

mod approved_handle_identity {
    use super::*;
    use crate::mutation::count::{
        CountMemory, CountProcesses, WindowsCountMemory, PC_V201_COUNT_LAYOUT,
    };
    use crate::mutation::live_fakes::{FixtureProcesses, InventoryFixture, FIXTURE_PID};
    use crate::mutation::memory::TargetProcess;
    use std::cell::Cell;
    use std::rc::Rc;

    #[derive(Default)]
    struct Observed {
        reads: Cell<usize>,
        writes: Cell<usize>,
        closes: Cell<usize>,
    }
    struct Handles {
        inner: FixtureProcesses,
        reader_creation: u64,
        writer_creation: u64,
        observed: Rc<Observed>,
    }
    struct Handle {
        inner: Box<dyn TargetProcess>,
        creation: u64,
        observed: Rc<Observed>,
    }
    impl CountProcesses for Handles {
        fn open_read(&self, pid: u32) -> Result<Box<dyn TargetProcess>, RuntimeError> {
            Ok(Box::new(Handle {
                inner: self.inner.open_read(pid)?,
                creation: self.reader_creation,
                observed: Rc::clone(&self.observed),
            }))
        }
        fn open_write(&self, pid: u32) -> Result<Box<dyn TargetProcess>, RuntimeError> {
            Ok(Box::new(Handle {
                inner: self.inner.open_write(pid)?,
                creation: self.writer_creation,
                observed: Rc::clone(&self.observed),
            }))
        }
    }
    impl TargetProcess for Handle {
        fn pid(&self) -> u32 {
            self.inner.pid()
        }
        fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
            self.observed.reads.set(self.observed.reads.get() + 1);
            self.inner.read(address, size)
        }
        fn write(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError> {
            self.observed.writes.set(self.observed.writes.get() + 1);
            self.inner.write(address, data)
        }
        fn write_code(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError> {
            self.inner.write_code(address, data)
        }
        fn allocate_executable_near(
            &mut self,
            address: u64,
            size: usize,
        ) -> Result<u64, RuntimeError> {
            self.inner.allocate_executable_near(address, size)
        }
        fn free_allocation(&mut self, address: u64) -> Result<(), RuntimeError> {
            self.inner.free_allocation(address)
        }
        fn exited(&mut self) -> Result<bool, RuntimeError> {
            self.inner.exited()
        }
        fn creation_filetime(&mut self) -> Result<Option<u64>, RuntimeError> {
            Ok(Some(self.creation))
        }
        fn close(&mut self) {
            self.observed.closes.set(self.observed.closes.get() + 1);
            self.inner.close()
        }
    }
    fn memory(
        reader_creation: u64,
        writer_creation: u64,
    ) -> (WindowsCountMemory<Handles>, Rc<Observed>) {
        let fixture = InventoryFixture::new(&[(4, 0x1234, 0xF00D)], 0x1235, 12);
        let observed = Rc::new(Observed::default());
        let handles = Handles {
            inner: FixtureProcesses::new(&fixture),
            reader_creation,
            writer_creation,
            observed: Rc::clone(&observed),
        };
        (
            WindowsCountMemory::new(FIXTURE_PID, fixture.base, PC_V201_COUNT_LAYOUT, handles)
                .with_expected_creation(100),
            observed,
        )
    }
    #[test]
    fn approved_count_identity_refuses_a_restarted_reader_before_memory_access() {
        let (mut memory, observed) = memory(200, 200);
        let error = memory
            .capture(0x1234)
            .expect_err("approval belongs to the previous process");
        assert!(error.message().contains("expected PID 4321 creation 100"));
        assert!(error
            .message()
            .contains("actual PID 4321 creation Some(200)"));
        assert!(error.message().contains("Reconnect"));
        assert_eq!(observed.reads.get(), 0);
        assert_eq!(observed.writes.get(), 0);
        assert_eq!(observed.closes.get(), 1);
    }
    #[test]
    fn approved_count_identity_rechecks_the_minimal_writer_then_allows_reprepare() {
        let (mut stale, observed) = memory(100, 200);
        let capture = stale
            .capture(0x1234)
            .expect("approved reader captures the reviewed record");
        let error = stale
            .write(&capture, 5)
            .expect_err("writer handle belongs to a later process");
        assert!(error
            .message()
            .contains("actual PID 4321 creation Some(200)"));
        assert_eq!(observed.writes.get(), 0);
        assert_eq!(observed.closes.get(), 1);
        let (mut fresh, observed) = memory(200, 200);
        fresh = fresh.with_expected_creation(200);
        let capture = fresh
            .capture(0x1234)
            .expect("newly reviewed process captures");
        assert_eq!(
            fresh.write(&capture, 5).expect("fresh plan writes once")[0x33],
            5
        );
        assert_eq!(observed.writes.get(), 1);
        assert_eq!(observed.closes.get(), 1);
    }
}

static SANDBOX_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// A temporary state root that removes itself.
struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Result<Self, RuntimeError> {
        let sequence = SANDBOX_SEQUENCE.fetch_add(1, Ordering::SeqCst);
        let mut root = std::env::temp_dir();
        root.push(format!(
            "nioh3-runtime-count-{name}-{}-{sequence}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).map_err(|error| RuntimeError::Io {
                path: root.display().to_string(),
                detail: error.to_string(),
            })?;
        }
        fs::create_dir_all(&root).map_err(|error| RuntimeError::Io {
            path: root.display().to_string(),
            detail: error.to_string(),
        })?;
        Ok(Self { root })
    }

    fn file(&self, name: &str, bytes: &[u8]) -> Result<PathBuf, RuntimeError> {
        let path = self.root.join(name);
        fs::write(&path, bytes).map_err(|error| RuntimeError::Io {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
        Ok(path)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

const SERIAL: u64 = 0x1122_3344_5566_7788;
const SEED: u32 = 0x0BAD_F00D;

/// One scroll record with the defined fields a count edit must preserve.
fn record() -> Vec<u8> {
    let mut bytes = vec![0u8; RECORD_SIZE];
    bytes[0x00] = 0x04;
    bytes[0x02] = 0x11;
    bytes[0x0E] = 0;
    bytes[0x18] = 0x03; // includes the new-item marker the edit must mask
    bytes[0x10] = 0x5A;
    bytes[0x20..0x24].copy_from_slice(&SEED.to_le_bytes());
    bytes[0x28..0x30].copy_from_slice(&SERIAL.to_le_bytes());
    bytes[0x30] = 4;
    bytes[0x33] = 2;
    bytes
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A prepared editor with its save and automatic backup on disk.
fn prepared(
    name: &str,
    faults: Faults,
    record_bytes: Vec<u8>,
) -> Result<(Sandbox, CountEditor, PathBuf), RuntimeError> {
    let sandbox = Sandbox::new(name)?;
    let save = sandbox.file("SAVEDATA.BIN", b"synthetic-save")?;
    let backup = sandbox.file("SAVEDATA.backup.BIN", b"synthetic-save")?;
    let mut editor = CountEditor::new(
        &sandbox.root,
        Box::new(FakeCountMemory::with_faults(record_bytes.clone(), faults)),
    )?;
    let save_digest = sha256_hex(b"synthetic-save");
    let status = editor.prepare(&save, &save_digest, &hex(&record_bytes), &backup, 5)?;
    assert_eq!(status.state, CountState::Prepared);
    Ok((sandbox, editor, save))
}

#[test]
fn prepare_records_the_planned_fields_and_a_verified_digest() -> Result<(), RuntimeError> {
    let (_sandbox, editor, _save) = prepared("prepare", Faults::default(), record())?;
    let (digest, plan) = first_plan(&editor)?;
    assert_eq!(plan.old_count, 2);
    assert_eq!(plan.new_count, 5);
    assert_eq!(plan.seed, SEED);
    assert_eq!(plan.rarity, 4);
    assert_eq!(plan.target.serial, SERIAL);
    assert_eq!(digest, plan.digest());
    let status = editor.status(&plan.operation_id)?;
    assert_eq!(status.state, CountState::Prepared);
    assert_eq!(status.plan_digest, digest);
    Ok(())
}

#[test]
fn checked_count_and_stable_identity_match_the_shipped_rules() -> Result<(), RuntimeError> {
    assert_eq!(checked_count(0)?, 0);
    assert_eq!(checked_count(7)?, 7);
    assert_eq!(
        checked_count(8).err(),
        Some(RuntimeError::InvalidCount { value: 8 })
    );
    assert_eq!(
        checked_count(-1).err(),
        Some(RuntimeError::InvalidCount { value: -1 })
    );

    let mut changed = record();
    changed[0x18] &= !2; // viewing a scroll clears the new-item marker
    changed[0x33] = 6; // the live count may differ from the checkpoint
    assert_eq!(
        stable_identity(&changed),
        stable_identity(&record()),
        "the masked fields do not participate in the identity"
    );
    let mut tampered = record();
    tampered[0x10] ^= 0xFF;
    assert_ne!(stable_identity(&tampered), stable_identity(&record()));
    Ok(())
}

#[test]
fn canonical_json_escapes_like_python() {
    let value = serde_json::json!({
        "b": 1,
        "a": "路径 \\ \" \n \u{1F600}",
    });
    assert_eq!(
        canonical_json(&value),
        "{\"a\":\"\\u8def\\u5f84 \\\\ \\\" \\n \\ud83d\\ude00\",\"b\":1}"
    );
}

#[test]
fn prepare_refuses_an_unsupported_current_state() -> Result<(), RuntimeError> {
    let sandbox = Sandbox::new("unsupported")?;
    let save = sandbox.file("SAVEDATA.BIN", b"synthetic-save")?;
    let backup = sandbox.file("SAVEDATA.backup.BIN", b"synthetic-save")?;
    let mut bytes = record();
    bytes[0x0E] = 1;
    let mut editor =
        CountEditor::new(&sandbox.root, Box::new(FakeCountMemory::new(bytes.clone())))?;
    let error = editor
        .prepare(
            &save,
            &sha256_hex(b"synthetic-save"),
            &hex(&bytes),
            &backup,
            3,
        )
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(
        error,
        RuntimeError::CountSourceChanged {
            detail: "Current scroll state is not supported for count editing".to_string(),
        }
    );
    Ok(())
}

#[test]
fn prepare_refuses_a_changed_save_or_backup() -> Result<(), RuntimeError> {
    let sandbox = Sandbox::new("changed")?;
    let save = sandbox.file("SAVEDATA.BIN", b"synthetic-save")?;
    let backup = sandbox.file("SAVEDATA.backup.BIN", b"another-save")?;
    let mut editor = CountEditor::new(&sandbox.root, Box::new(FakeCountMemory::new(record())))?;
    let error = editor
        .prepare(
            &save,
            &sha256_hex(b"synthetic-save"),
            &hex(&record()),
            &backup,
            3,
        )
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert!(
        matches!(error, RuntimeError::BackupMismatch { .. }),
        "a backup that does not match the save is refused: {error:?}"
    );

    let error = editor
        .prepare(
            &save,
            &sha256_hex(b"stale-save"),
            &hex(&record()),
            &backup,
            3,
        )
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(
        error,
        RuntimeError::CountSourceChanged {
            detail: "Save changed; refresh inventory".to_string(),
        }
    );
    Ok(())
}

#[test]
fn execute_writes_one_byte_reads_it_back_and_resolves_verified() -> Result<(), RuntimeError> {
    let (_sandbox, mut editor, _save) = prepared("execute", Faults::default(), record())?;
    let (_digest, plan) = first_plan(&editor)?;
    let status = editor.execute(&plan.operation_id, &plan.digest())?;
    assert_eq!(status.state, CountState::Verified);
    assert_eq!(status.old_count, 2);
    assert_eq!(status.new_count, 5);
    let directory = editor.directory(&plan.operation_id)?;
    assert!(directory.join("claim.json").exists());
    assert!(directory.join("verified-record.json").exists());
    assert!(directory.join("receipt.json").exists());

    // A second execute is a no-op: the receipt is already resolved.
    let again = editor.execute(&plan.operation_id, &plan.digest())?;
    assert_eq!(again.state, CountState::Verified);
    Ok(())
}

fn first_plan(editor: &CountEditor) -> Result<(String, super::count::CountPlan), RuntimeError> {
    let root = editor.operations_directory();
    let mut found = None;
    for entry in fs::read_dir(&root).map_err(|error| RuntimeError::Io {
        path: root.display().to_string(),
        detail: error.to_string(),
    })? {
        let entry = entry.map_err(|error| RuntimeError::Io {
            path: root.display().to_string(),
            detail: error.to_string(),
        })?;
        if entry.path().is_dir() {
            let id = entry.file_name().to_string_lossy().to_string();
            found = Some(editor.plan(&id)?);
        }
    }
    found.ok_or(RuntimeError::CountInstanceChanged)
}

#[test]
fn a_readback_mismatch_is_uncertain_and_never_replayed() -> Result<(), RuntimeError> {
    let (_sandbox, mut editor, _save) = prepared(
        "readback",
        Faults {
            // The target keeps its own bytes, so the readback differs from the
            // record the editor expected to see.
            quiet_writes: true,
            ..Faults::default()
        },
        record(),
    )?;
    let (digest, plan) = first_plan(&editor)?;
    let status = editor.execute(&plan.operation_id, &digest)?;
    assert_eq!(status.state, CountState::Uncertain);
    assert_eq!(
        status.error.as_deref(),
        Some("Count write readback differs; inspect before retrying")
    );

    // Replaying would be a second write; the editor must refuse instead.
    let status = editor.execute(&plan.operation_id, &digest)?;
    assert_eq!(status.state, CountState::Uncertain);

    // Recovery only observes. The record still holds the old count, so the
    // uncertain receipt stays.
    let status = editor.recover(&plan.operation_id)?;
    assert_eq!(status.state, CountState::Uncertain);
    assert!(!editor
        .directory(&plan.operation_id)?
        .join("recovery.json")
        .exists());
    Ok(())
}

#[test]
fn recovery_resolves_an_uncertain_receipt_when_the_write_landed() -> Result<(), RuntimeError> {
    // The write reached the record but the reply was lost, which is exactly the
    // case `recover` exists for: observe, never replay.
    let (_sandbox, mut editor, _save) = prepared(
        "recover-landed",
        Faults {
            readback_after_write: Some(record()),
            ..Faults::default()
        },
        record(),
    )?;
    let (digest, plan) = first_plan(&editor)?;
    let status = editor.execute(&plan.operation_id, &digest)?;
    assert_eq!(status.state, CountState::Uncertain);

    let status = editor.recover(&plan.operation_id)?;
    assert_eq!(status.state, CountState::Verified);
    assert_eq!(status.error, None);
    assert!(editor
        .directory(&plan.operation_id)?
        .join("recovery.json")
        .exists());
    Ok(())
}

#[test]
fn a_failed_write_is_uncertain_because_the_target_may_have_changed() -> Result<(), RuntimeError> {
    let (_sandbox, mut editor, _save) = prepared(
        "writefail",
        Faults {
            fail_write_call: Some(1),
            ..Faults::default()
        },
        record(),
    )?;
    let (digest, plan) = first_plan(&editor)?;
    let status = editor.execute(&plan.operation_id, &digest)?;
    assert_eq!(status.state, CountState::Uncertain);
    assert!(status.error.is_some());
    Ok(())
}

#[test]
fn a_changed_source_is_rejected_before_any_write() -> Result<(), RuntimeError> {
    let (_sandbox, mut editor, save) = prepared("source", Faults::default(), record())?;
    let (digest, plan) = first_plan(&editor)?;
    fs::write(&save, b"tampered-save").map_err(|error| RuntimeError::Io {
        path: save.display().to_string(),
        detail: error.to_string(),
    })?;
    let status = editor.execute(&plan.operation_id, &digest)?;
    assert_eq!(status.state, CountState::Rejected);
    assert_eq!(
        status.error.as_deref(),
        Some("Save or automatic backup changed; prepare again")
    );
    Ok(())
}

#[test]
fn recovery_verifies_a_lost_reply_without_writing() -> Result<(), RuntimeError> {
    let (_sandbox, editor, _save) = prepared("recover", Faults::default(), record())?;
    let (digest, plan) = first_plan(&editor)?;
    let directory = editor.directory(&plan.operation_id)?;

    // A claim whose reply never arrived: the write happened, the receipt did not.
    fs::write(
        directory.join("claim.json"),
        format!("{{\"digest\":\"{digest}\"}}"),
    )
    .map_err(|error| RuntimeError::Io {
        path: directory.display().to_string(),
        detail: error.to_string(),
    })?;
    assert_eq!(
        editor.status(&plan.operation_id)?.state,
        CountState::Uncertain
    );

    let mut written = record();
    written[0x33] = plan.new_count;
    let mut editor = replace_memory(editor, written);
    let status = editor.recover(&plan.operation_id)?;
    assert_eq!(status.state, CountState::Verified);
    assert!(directory.join("recovery.json").exists());
    Ok(())
}

fn replace_memory(editor: CountEditor, record_bytes: Vec<u8>) -> CountEditor {
    editor.with_memory(Box::new(FakeCountMemory::new(record_bytes)))
}

#[test]
fn recovery_refuses_a_different_instance() -> Result<(), RuntimeError> {
    let (_sandbox, editor, _save) = prepared("instance", Faults::default(), record())?;
    let (digest, plan) = first_plan(&editor)?;
    let directory = editor.directory(&plan.operation_id)?;
    fs::write(
        directory.join("claim.json"),
        format!("{{\"digest\":\"{digest}\"}}"),
    )
    .map_err(|error| RuntimeError::Io {
        path: directory.display().to_string(),
        detail: error.to_string(),
    })?;

    let mut other = FakeCountMemory::new(record());
    other.pid += 1;
    let mut editor = editor.with_memory(Box::new(other));
    let error = editor
        .recover(&plan.operation_id)
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(error, RuntimeError::CountInstanceChanged);
    Ok(())
}

#[test]
fn an_unreviewed_digest_is_refused() -> Result<(), RuntimeError> {
    let (_sandbox, mut editor, _save) = prepared("digest", Faults::default(), record())?;
    let (_digest, plan) = first_plan(&editor)?;
    let error = editor
        .execute(&plan.operation_id, "0".repeat(64).as_str())
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(
        error,
        RuntimeError::ReceiptConflict {
            detail: "Reviewed count plan digest differs".to_string(),
        }
    );
    Ok(())
}

/// A record edit that changes the seed and one effect value, built from the
/// live record so its count byte and new-item mark are the game's.
fn reseed(current: &[u8]) -> Result<Vec<u8>, String> {
    let mut desired = current.to_vec();
    desired[0x20..0x24].copy_from_slice(&0x0123_4567u32.to_le_bytes());
    desired[0x40] = 0x2A;
    Ok(desired)
}

fn prepared_record(
    name: &str,
    faults: Faults,
    saved: Vec<u8>,
    live: Vec<u8>,
) -> Result<(Sandbox, CountEditor), RuntimeError> {
    let sandbox = Sandbox::new(name)?;
    let save = sandbox.file("SAVEDATA.BIN", b"synthetic-save")?;
    let backup = sandbox.file("SAVEDATA.backup.BIN", b"synthetic-save")?;
    let mut editor = CountEditor::new(
        &sandbox.root,
        Box::new(FakeCountMemory::with_faults(live, faults)),
    )?;
    let status = editor.prepare_record(
        &save,
        &sha256_hex(b"synthetic-save"),
        &hex(&saved),
        &backup,
        &reseed,
    )?;
    assert_eq!(status.state, CountState::Prepared);
    Ok((sandbox, editor))
}

#[test]
fn a_record_edit_writes_the_reviewed_record_built_from_the_live_one() -> Result<(), RuntimeError> {
    // The game counted one attempt down since the save was read.
    let mut live = record();
    live[0x33] = 1;
    let (_sandbox, mut editor) =
        prepared_record("record", Faults::default(), record(), live.clone())?;
    let (digest, plan) = first_plan(&editor)?;
    let desired = reseed(&live).map_err(|detail| RuntimeError::CountSourceChanged { detail })?;
    assert_eq!(
        plan.desired_record_hex.as_deref(),
        Some(hex(&desired).as_str())
    );
    assert_eq!(plan.seed, 0x0123_4567);
    assert_eq!(plan.old_count, 1);
    let status = editor.execute(&plan.operation_id, &digest)?;
    assert_eq!(status.state, CountState::Verified);
    let written = fs::read_to_string(
        editor
            .directory(&plan.operation_id)?
            .join("verified-record.json"),
    )
    .map_err(|error| RuntimeError::Io {
        path: "verified-record.json".into(),
        detail: error.to_string(),
    })?;
    assert!(written.contains(&hex(&desired)));
    Ok(())
}

#[test]
fn a_count_plan_digest_is_unchanged_by_the_record_field() -> Result<(), RuntimeError> {
    let (_sandbox, editor, _save) = prepared("count-shape", Faults::default(), record())?;
    let (digest, plan) = first_plan(&editor)?;
    assert!(plan.desired_record_hex.is_none());
    assert!(plan.to_json().get("desired_record_hex").is_none());
    assert_eq!(
        digest,
        sha256_hex(canonical_json(&plan.to_json()).as_bytes())
    );
    Ok(())
}

#[test]
fn a_record_edit_refuses_no_change_and_a_serial_change() -> Result<(), RuntimeError> {
    let sandbox = Sandbox::new("record-refuse")?;
    let save = sandbox.file("SAVEDATA.BIN", b"synthetic-save")?;
    let backup = sandbox.file("SAVEDATA.backup.BIN", b"synthetic-save")?;
    let mut editor = CountEditor::new(&sandbox.root, Box::new(FakeCountMemory::new(record())))?;
    let digest = sha256_hex(b"synthetic-save");
    let same = |current: &[u8]| Ok(current.to_vec());
    assert!(matches!(
        editor.prepare_record(&save, &digest, &hex(&record()), &backup, &same),
        Err(RuntimeError::CountSourceChanged { .. })
    ));
    let serial = |current: &[u8]| {
        let mut desired = current.to_vec();
        desired[0x28] ^= 1;
        Ok(desired)
    };
    assert!(matches!(
        editor.prepare_record(&save, &digest, &hex(&record()), &backup, &serial),
        Err(RuntimeError::CountSourceChanged { .. })
    ));
    assert!(fs::read_dir(editor.operations_directory())
        .map_err(|error| RuntimeError::Io {
            path: "count-edits".into(),
            detail: error.to_string()
        })?
        .next()
        .is_none());
    Ok(())
}

#[test]
fn a_record_edit_readback_mismatch_is_uncertain_and_recovery_observes_it(
) -> Result<(), RuntimeError> {
    let (_sandbox, mut editor) = prepared_record(
        "record-readback",
        Faults {
            readback_after_write: Some(record()),
            ..Faults::default()
        },
        record(),
        record(),
    )?;
    let (digest, plan) = first_plan(&editor)?;
    let status = editor.execute(&plan.operation_id, &digest)?;
    assert_eq!(status.state, CountState::Uncertain);
    // The record really holds the reviewed bytes, with a count the game changed.
    let mut landed =
        reseed(&record()).map_err(|detail| RuntimeError::CountSourceChanged { detail })?;
    landed[0x33] = 0;
    let mut editor = replace_memory(editor, landed);
    assert_eq!(
        editor.recover(&plan.operation_id)?.state,
        CountState::Verified
    );
    Ok(())
}

#[test]
fn a_scroll_this_editor_rewrote_stays_editable_until_the_game_saves() -> Result<(), RuntimeError> {
    let (sandbox, mut editor) = prepared_record("chain", Faults::default(), record(), record())?;
    let (digest, plan) = first_plan(&editor)?;
    assert_eq!(
        editor.execute(&plan.operation_id, &digest)?.state,
        CountState::Verified
    );
    // The game now holds the rewritten record; the save still holds the old one.
    let live = reseed(&record()).map_err(|detail| RuntimeError::CountSourceChanged { detail })?;
    let mut editor = replace_memory(editor, live.clone());
    let save = sandbox.root.join("SAVEDATA.BIN");
    let backup = sandbox.root.join("SAVEDATA.backup.BIN");
    let digest = sha256_hex(b"synthetic-save");
    let again = |current: &[u8]| {
        let mut desired = current.to_vec();
        desired[0x44] = 7;
        Ok(desired)
    };
    assert_eq!(
        editor
            .prepare_record(&save, &digest, &hex(&record()), &backup, &again)?
            .state,
        CountState::Prepared
    );
    assert_eq!(
        editor
            .prepare(&save, &digest, &hex(&record()), &backup, 3)?
            .state,
        CountState::Prepared
    );
    // A record the editor never wrote is still refused.
    let mut foreign = live;
    foreign[0x50] = 9;
    let mut editor = replace_memory(editor, foreign);
    assert!(matches!(
        editor.prepare(&save, &digest, &hex(&record()), &backup, 3),
        Err(RuntimeError::CountSourceChanged { .. })
    ));
    Ok(())
}
