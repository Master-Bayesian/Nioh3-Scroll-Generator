//! Durable one-shot receipts for title prestige edits. Recovery only observes.
use crate::HostError;
use nioh3_runtime::character::LiveEditOutcome;
use nioh3_runtime::prestige::Snapshot;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn fail(detail: impl Into<String>) -> HostError {
    HostError::coded("PRESTIGE_REFUSED", detail)
}
use crate::player_receipts::load;
pub(crate) use crate::player_receipts::operation_id;
fn directory(root: &Path) -> PathBuf {
    root.join("prestige-points")
}
fn path(root: &Path, id: &str) -> PathBuf {
    directory(root).join(format!("{id}.json"))
}
fn store(root: &Path, value: &Value) -> Result<(), HostError> {
    crate::player_receipts::store(root, "prestige-points", value)
}
fn public(receipt: Value) -> Value {
    json!({"prestige_edit":{
        "operation_id":receipt["operation_id"],"state":receipt["state"],
        "category_id":receipt["category_id"],"before":receipt["before"],"after":receipt["after"],
        "process_id":receipt["process_id"],"process_birth":receipt["process_birth"],
        "error":receipt["error"]
    }})
}
pub fn status(root: &Path, params: &Value) -> Result<Value, HostError> {
    if params.as_object().is_some_and(|p| p.is_empty()) {
        if directory(root)
            .try_exists()
            .map_err(|e| fail(e.to_string()))?
        {
            for (count, file) in std::fs::read_dir(directory(root))
                .map_err(|e| fail(e.to_string()))?
                .enumerate()
            {
                if count >= 8192 {
                    return Err(fail("Prestige receipt history exceeds bounds"));
                }
                let file = file.map_err(|e| fail(e.to_string()))?.path();
                if file.extension().is_some_and(|e| e == "json") {
                    let receipt = load(&file)?;
                    if receipt["state"] == "uncertain" {
                        return Ok(public(receipt));
                    }
                }
            }
        }
        return Ok(
            json!({"prestige_edit":{"operation_id":null,"state":"rejected",
            "category_id":null,"before":null,"after":null,"process_id":null,"process_birth":null,"error":null}}),
        );
    }
    let id = operation_id(params)?;
    if !path(root, id)
        .try_exists()
        .map_err(|e| fail(e.to_string()))?
    {
        // A temp alone never admitted a write. Every attempted game write has a
        // durable JSON receipt; a failed settlement leaves that receipt uncertain.
        return Ok(
            json!({"prestige_edit":{"operation_id":id,"state":"rejected",
            "category_id":null,"before":null,"after":null,"process_id":null,"process_birth":null,
            "error":"No recorded prestige write"}}),
        );
    }
    Ok(public(load(&path(root, id))?))
}
pub fn replay(root: &Path, params: &Value) -> Result<Option<Value>, HostError> {
    let id = operation_id(params)?;
    if !path(root, id)
        .try_exists()
        .map_err(|e| fail(e.to_string()))?
    {
        return Ok(None);
    }
    let saved = load(&path(root, id))?;
    let digest = format!("{:x}", Sha256::digest(params.to_string()));
    if saved["request_digest"] != digest {
        return Err(fail("Operation id belongs to another request"));
    }
    Ok(Some(public(saved)))
}
pub fn execute(
    root: &Path,
    params: &Value,
    read: impl FnOnce() -> Result<Snapshot, HostError>,
    write: impl FnOnce(&Snapshot, u32, u32) -> LiveEditOutcome,
) -> Result<Value, HostError> {
    let _lock = nioh3_runtime::mutation::native_executor::AdmissionLock::acquire(
        &root.join("player-edit-admission"),
    )
    .map_err(HostError::from_runtime)?;
    if let Some(saved) = replay(root, params)? {
        return Ok(saved);
    }
    crate::player_receipts::assert_no_pending(root)?;
    let id = operation_id(params)?;
    if params["confirmed"] != true {
        return Err(HostError::invalid_request());
    }
    let category = params["category_id"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(HostError::invalid_request)?;
    let available = params["available"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(HostError::invalid_request)?;
    std::fs::create_dir_all(directory(root)).map_err(|e| fail(e.to_string()))?;
    let mut count = 0;
    for file in std::fs::read_dir(directory(root)).map_err(|e| fail(e.to_string()))? {
        count += 1;
        if count > 8192 {
            return Err(fail("Prestige receipt history exceeds bounds"));
        }
        let file = file.map_err(|e| fail(e.to_string()))?.path();
        if file.extension().is_some_and(|e| e == "json") && load(&file)?["state"] == "uncertain" {
            return Err(HostError::coded(
                "PRESTIGE_RECOVERY_REQUIRED",
                "Check the previous prestige operation before another write",
            ));
        }
    }
    let before = read()?;
    if params["snapshot_id"] != before.id()
        || params["process_id"] != before.pid
        || params["process_birth"]
            .as_str()
            .and_then(|value| value.parse::<u64>().ok())
            != Some(before.birth)
    {
        return Err(HostError::coded(
            "PRESTIGE_STALE",
            "Reload prestige points before reviewing another change",
        ));
    }
    let after = before
        .replacement(category, available)
        .map_err(HostError::from_runtime)?;
    let row = before
        .categories
        .iter()
        .find(|r| r.id == category)
        .ok_or_else(HostError::invalid_request)?;
    let mut receipt = json!({
        "operation_id":id,"state":"uncertain","request_digest":format!("{:x}",Sha256::digest(params.to_string())),
        "process_id":before.pid,"process_birth":before.birth.to_string(),"category_id":category,
        "before":row.earned-row.spent,"after":available,"before_id":before.id(),"after_id":after.id(),
        "before_earned":row.earned,"spent":row.spent,"after_earned":row.spent+available,
        "error":null
    });
    store(root, &receipt)?;
    match write(&before, category, available) {
        LiveEditOutcome::Verified => receipt["state"] = json!("verified"),
        LiveEditOutcome::Rejected(error) => {
            receipt["state"] = json!("rejected");
            receipt["error"] = json!(error);
        }
        LiveEditOutcome::Uncertain(error) => receipt["error"] = json!(error),
    }
    // If settling fails, the durable uncertain receipt blocks further writes.
    store(root, &receipt)?;
    Ok(public(receipt))
}
pub fn recover(
    root: &Path,
    params: &Value,
    read: impl FnOnce() -> Result<Snapshot, HostError>,
    process_gone: impl FnOnce(u32, u64) -> Result<bool, HostError>,
) -> Result<Value, HostError> {
    let _lock = nioh3_runtime::mutation::native_executor::AdmissionLock::acquire(
        &root.join("player-edit-admission"),
    )
    .map_err(HostError::from_runtime)?;
    let id = operation_id(params)?;
    if !path(root, id)
        .try_exists()
        .map_err(|e| fail(e.to_string()))?
    {
        return status(root, params);
    }
    let mut receipt = load(&path(root, id))?;
    if receipt["state"] != "uncertain" {
        return Ok(public(receipt));
    }
    let pid = receipt["process_id"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(HostError::invalid_request)?;
    let birth = receipt["process_birth"]
        .as_str()
        .and_then(|n| n.parse::<u64>().ok())
        .ok_or_else(HostError::invalid_request)?;
    if process_gone(pid, birth)? {
        receipt["state"] = json!("expired");
    } else {
        let actual = read()?;
        if actual.pid != pid || actual.birth != birth {
            return Err(fail("Recorded process differs"));
        }
        let observed = actual.id();
        if receipt["after_id"] == observed {
            receipt["state"] = json!("verified");
        } else if receipt["before_id"] == observed {
            receipt["state"] = json!("not_applied");
        }
    }
    store(root, &receipt)?;
    Ok(public(receipt))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use nioh3_runtime::prestige::{snapshot, test_fixture::Memory};
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU64, Ordering};
    struct Scratch(PathBuf);
    static SCRATCH_ID: AtomicU64 = AtomicU64::new(0);
    impl Scratch {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "nioh3-prestige-test-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                SCRATCH_ID.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            if self.0.parent() == Some(std::env::temp_dir().as_path()) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }
    fn params() -> Value {
        let read = snapshot(&Memory::fixture()).unwrap();
        json!({"operation_id":"11111111-1111-4111-8111-111111111111","confirmed":true,
            "process_id":42,"process_birth":"77","snapshot_id":read.id(),"category_id":50152,"available":123})
    }
    #[test]
    fn receipt_is_durable_before_dispatch_and_duplicate_uuid_never_replays() {
        let root = Scratch::new();
        let p = params();
        let called = Cell::new(0);
        let result = execute(
            &root.0,
            &p,
            || Ok(snapshot(&Memory::fixture()).unwrap()),
            |_, _, _| {
                assert_eq!(
                    load(&path(&root.0, p["operation_id"].as_str().unwrap())).unwrap()["state"],
                    "uncertain"
                );
                called.set(called.get() + 1);
                LiveEditOutcome::Verified
            },
        )
        .unwrap();
        assert_eq!(result["prestige_edit"]["state"], "verified");
        let retry = execute(
            &root.0,
            &p,
            || panic!("replay must not read game"),
            |_, _, _| panic!("replay must not write"),
        )
        .unwrap();
        assert_eq!(result, retry);
        assert_eq!(called.get(), 1);
    }
    #[test]
    fn uncertain_receipt_survives_restart_blocks_fresh_ids_and_recovers_without_writing() {
        let root = Scratch::new();
        let p = params();
        execute(
            &root.0,
            &p,
            || Ok(snapshot(&Memory::fixture()).unwrap()),
            |_, _, _| LiveEditOutcome::Uncertain("injected partial write".into()),
        )
        .unwrap();
        assert_eq!(
            status(&root.0, &p).unwrap()["prestige_edit"]["state"],
            "uncertain"
        );
        assert_eq!(
            status(&root.0, &json!({})).unwrap()["prestige_edit"]["operation_id"],
            p["operation_id"]
        );
        let mut another = p.clone();
        another["operation_id"] = json!("22222222-2222-4222-8222-222222222222");
        assert!(execute(
            &root.0,
            &another,
            || panic!("pending receipt must block reads"),
            |_, _, _| panic!("pending receipt must block writes")
        )
        .is_err());
        let recovered = recover(
            &root.0,
            &p,
            || Ok(snapshot(&Memory::fixture()).unwrap()),
            |_, _| Ok(false),
        )
        .unwrap();
        assert_eq!(recovered["prestige_edit"]["state"], "not_applied");
    }
    #[test]
    fn reused_id_changed_request_and_stale_snapshot_never_dispatch() {
        let root = Scratch::new();
        let mut p = params();
        p["snapshot_id"] = json!("a".repeat(64));
        assert!(execute(
            &root.0,
            &p,
            || Ok(snapshot(&Memory::fixture()).unwrap()),
            |_, _, _| panic!("stale")
        )
        .is_err());
        let mut p = params();
        execute(
            &root.0,
            &p,
            || Ok(snapshot(&Memory::fixture()).unwrap()),
            |_, _, _| LiveEditOutcome::Rejected("preflight refused".into()),
        )
        .unwrap();
        p["available"] = json!(124);
        assert!(execute(
            &root.0,
            &p,
            || panic!("changed uuid"),
            |_, _, _| panic!("changed uuid")
        )
        .is_err());
    }
    #[test]
    fn confirmed_process_exit_resolves_uncertainty_without_game_access() {
        let root = Scratch::new();
        let p = params();
        execute(
            &root.0,
            &p,
            || Ok(snapshot(&Memory::fixture()).unwrap()),
            |_, _, _| LiveEditOutcome::Uncertain("lost".into()),
        )
        .unwrap();
        let result = recover(
            &root.0,
            &p,
            || panic!("dead process must not be read"),
            |pid, birth| {
                assert_eq!((pid, birth), (42, 77));
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(result["prestige_edit"]["state"], "expired");
    }
    #[test]
    fn malformed_ids_corrupt_receipts_and_failed_durability_fail_before_writes() {
        let root = Scratch::new();
        for id in ["../escape", "A".repeat(36).as_str(), "x"] {
            assert!(status(&root.0, &json!({"operation_id":id})).is_err());
        }
        let p = params();
        std::fs::create_dir_all(directory(&root.0)).unwrap();
        std::fs::write(
            path(&root.0, "22222222-2222-4222-8222-222222222222"),
            b"broken",
        )
        .unwrap();
        assert!(execute(
            &root.0,
            &p,
            || panic!("corrupt"),
            |_, _, _| panic!("corrupt")
        )
        .is_err());
        let root = Scratch::new();
        std::fs::create_dir_all(
            path(&root.0, p["operation_id"].as_str().unwrap()).with_extension("tmp"),
        )
        .unwrap();
        assert!(execute(
            &root.0,
            &p,
            || Ok(snapshot(&Memory::fixture()).unwrap()),
            |_, _, _| panic!("not durable")
        )
        .is_err());
    }
    #[test]
    fn crash_left_temp_alone_did_not_admit_a_game_write() {
        let root = Scratch::new();
        let p = params();
        std::fs::create_dir_all(directory(&root.0)).unwrap();
        std::fs::write(
            path(&root.0, p["operation_id"].as_str().unwrap()).with_extension("tmp"),
            b"partial",
        )
        .unwrap();
        assert_eq!(
            status(&root.0, &p).unwrap()["prestige_edit"]["state"],
            "rejected"
        );
        assert_eq!(
            execute(
                &root.0,
                &p,
                || Ok(snapshot(&Memory::fixture()).unwrap()),
                |_, _, _| LiveEditOutcome::Verified
            )
            .unwrap()["prestige_edit"]["state"],
            "verified"
        );
    }
}
