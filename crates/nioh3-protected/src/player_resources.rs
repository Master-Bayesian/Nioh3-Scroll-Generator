//! Persistent resource edit receipts; recovery only reads the recorded state.
use crate::{
    player_receipts::{load, operation_id, store},
    HostError,
};
use nioh3_runtime::{character::LiveEditOutcome, player_resources::Snapshot};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
const NAMESPACE: &str = "player-resources";
fn fail(text: impl Into<String>) -> HostError {
    HostError::coded("PLAYER_RESOURCE_REFUSED", text)
}
fn path(root: &Path, id: &str) -> PathBuf {
    root.join(NAMESPACE).join(format!("{id}.json"))
}
fn public(r: Value) -> Value {
    json!({"player_edit":{
        "operation_id":r["operation_id"],"state":r["state"],"field":r["field"],"clan_id":r["clan_id"],
        "before":r["before"],"after":r["after"],"error":r["error"]
    }})
}
pub fn status(root: &Path, params: &Value) -> Result<Value, HostError> {
    if params.as_object().is_some_and(|p| p.is_empty()) {
        let dir = root.join(NAMESPACE);
        if dir.try_exists().map_err(|e| fail(e.to_string()))? {
            for (count, file) in std::fs::read_dir(dir)
                .map_err(|e| fail(e.to_string()))?
                .enumerate()
            {
                if count >= 8192 {
                    return Err(fail("Resource edit history exceeds bounds"));
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
        return Ok(public(
            json!({"operation_id":null,"state":"rejected","field":null,"clan_id":0,"before":null,"after":null,"error":null}),
        ));
    }
    let id = operation_id(params)?;
    if !path(root, id)
        .try_exists()
        .map_err(|e| fail(e.to_string()))?
    {
        return Ok(public(
            json!({"operation_id":id,"state":"rejected","field":null,"clan_id":0,
            "before":null,"after":null,"error":"No recorded resource write"}),
        ));
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
    let r = load(&path(root, id))?;
    if r["request_digest"] != format!("{:x}", Sha256::digest(params.to_string())) {
        return Err(fail("Operation ID belongs to another request"));
    }
    Ok(Some(public(r)))
}
pub fn execute(
    root: &Path,
    params: &Value,
    read: impl FnOnce() -> Result<Snapshot, HostError>,
    write: impl FnOnce(&Snapshot, &str, u32, u64) -> LiveEditOutcome,
) -> Result<Value, HostError> {
    let _lock = nioh3_runtime::mutation::native_executor::AdmissionLock::acquire(
        &root.join("player-edit-admission"),
    )
    .map_err(HostError::from_runtime)?;
    if let Some(r) = replay(root, params)? {
        return Ok(r);
    }
    crate::player_receipts::assert_no_pending(root)?;
    let id = operation_id(params)?;
    if params["confirmed"] != true {
        return Err(HostError::invalid_request());
    }
    let field = params["field"]
        .as_str()
        .ok_or_else(HostError::invalid_request)?;
    let clan = params["clan_id"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(HostError::invalid_request)?;
    let value = params["value"]
        .as_str()
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 20
                && s.bytes().all(|b| b.is_ascii_digit())
                && (*s == "0" || !s.starts_with('0'))
        })
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or_else(HostError::invalid_request)?;
    let before = read()?;
    if params["snapshot_id"] != before.id()
        || params["process_id"] != before.pid
        || params["process_birth"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            != Some(before.birth)
    {
        return Err(fail(
            "Reload player resources before reviewing another change",
        ));
    }
    let (after, patches) = before
        .plan(field, clan, value)
        .map_err(HostError::from_runtime)?;
    let original = before.value(field, clan).map_err(HostError::from_runtime)?;
    let mut receipt = json!({"operation_id":id,"state":"uncertain",
        "request_digest":format!("{:x}",Sha256::digest(params.to_string())),
        "process_id":before.pid,"process_birth":before.birth.to_string(),"field":field,"clan_id":clan,
        "before":original.to_string(),"after":value.to_string(),"before_id":before.id(),"after_id":after.id(),
        "patches":patches.iter().map(|p|json!({"before":p.before,"after":p.after})).collect::<Vec<_>>(),"error":null});
    store(root, NAMESPACE, &receipt)?;
    match write(&before, field, clan, value) {
        LiveEditOutcome::Verified => receipt["state"] = json!("verified"),
        LiveEditOutcome::Rejected(error) => {
            receipt["state"] = json!("rejected");
            receipt["error"] = json!(error);
        }
        LiveEditOutcome::Uncertain(error) => receipt["error"] = json!(error),
    }
    store(root, NAMESPACE, &receipt)?;
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
    let mut r = load(&path(root, id))?;
    if r["state"] != "uncertain" {
        return Ok(public(r));
    }
    let pid = r["process_id"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(HostError::invalid_request)?;
    let birth = r["process_birth"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or_else(HostError::invalid_request)?;
    if process_gone(pid, birth)? {
        r["state"] = json!("expired");
    } else {
        let actual = read()?;
        if actual.pid != pid || actual.birth != birth {
            return Err(fail("Recorded process differs"));
        }
        let id = actual.id();
        if r["after_id"] == id {
            r["state"] = json!("verified");
        } else if r["before_id"] == id {
            r["state"] = json!("not_applied");
        }
    }
    store(root, NAMESPACE, &r)?;
    Ok(public(r))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use nioh3_runtime::player_resources::{
        apply_at, snapshot_at,
        test_fixture::{Memory, Writer, NOW},
    };
    use std::sync::atomic::{AtomicU64, Ordering};
    static ID: AtomicU64 = AtomicU64::new(0);
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "nioh3-resource-test-{}-{}",
                std::process::id(),
                ID.fetch_add(1, Ordering::Relaxed)
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
    fn params(m: &Memory) -> Value {
        let s = snapshot_at(m, NOW).unwrap();
        json!({"operation_id":"11111111-1111-4111-8111-111111111111","confirmed":true,
            "process_id":s.pid,"process_birth":s.birth.to_string(),"snapshot_id":s.id(),
            "field":"glory","clan_id":0,"value":"100"})
    }
    #[test]
    fn durable_before_write_exact_glory_mirrors_and_idempotent_replay() {
        let root = Scratch::new();
        let m = Memory::fixture();
        let p = params(&m);
        let result = execute(
            &root.0,
            &p,
            || Ok(snapshot_at(&m, NOW).unwrap()),
            |before, field, clan, value| {
                assert_eq!(
                    load(&path(&root.0, p["operation_id"].as_str().unwrap())).unwrap()["state"],
                    "uncertain"
                );
                let mut writer = Writer::new(&m);
                let r = apply_at(&m, &mut writer, before, field, clan, value, NOW);
                assert_eq!(writer.writes.len(), 2);
                r
            },
        )
        .unwrap();
        assert_eq!(result["player_edit"]["state"], "verified");
        assert_eq!(
            result,
            execute(
                &root.0,
                &p,
                || panic!("replay read"),
                |_, _, _, _| panic!("replay write")
            )
            .unwrap()
        );
        let mut changed = p;
        changed["value"] = json!("101");
        assert!(replay(&root.0, &changed).is_err());
    }
    #[test]
    fn uncertain_resource_edit_blocks_prestige_and_new_resource_edits_and_is_discovered() {
        let root = Scratch::new();
        let m = Memory::fixture();
        let p = params(&m);
        execute(
            &root.0,
            &p,
            || Ok(snapshot_at(&m, NOW).unwrap()),
            |_, _, _, _| LiveEditOutcome::Uncertain("response loss".into()),
        )
        .unwrap();
        assert_eq!(
            status(&root.0, &json!({})).unwrap()["player_edit"]["operation_id"],
            p["operation_id"]
        );
        let mut fresh = p.clone();
        fresh["operation_id"] = json!("22222222-2222-4222-8222-222222222222");
        assert!(execute(
            &root.0,
            &fresh,
            || panic!("pending read"),
            |_, _, _, _| panic!("pending write")
        )
        .is_err());
        let prestige_memory = nioh3_runtime::prestige::test_fixture::Memory::fixture();
        let ps = nioh3_runtime::prestige::snapshot(&prestige_memory).unwrap();
        let pp = json!({"operation_id":"33333333-3333-4333-8333-333333333333","confirmed":true,
            "process_id":42,"process_birth":"77","snapshot_id":ps.id(),"category_id":50152,"available":123});
        assert!(crate::prestige::execute(
            &root.0,
            &pp,
            || panic!("cross-scope read"),
            |_, _, _| panic!("cross-scope write")
        )
        .is_err());
        let recovered = recover(
            &root.0,
            &p,
            || Ok(snapshot_at(&m, NOW).unwrap()),
            |_, _| Ok(false),
        )
        .unwrap();
        assert_eq!(recovered["player_edit"]["state"], "not_applied");
        crate::player_receipts::assert_no_pending(&root.0).unwrap();
    }
    #[test]
    fn changed_snapshot_value_types_caps_and_unconfirmed_requests_never_dispatch() {
        let root = Scratch::new();
        let m = Memory::fixture();
        let p = params(&m);
        for (key, value) in [
            ("value", json!("2000000001")),
            ("value", json!("18446744073709551616")),
            ("value", json!("01")),
            ("value", json!(99)),
            ("confirmed", json!(false)),
            ("snapshot_id", json!("0".repeat(64))),
            ("field", json!("address")),
            ("clan_id", json!(7)),
        ] {
            let mut bad = p.clone();
            bad[key] = value;
            assert!(execute(
                &root.0,
                &bad,
                || Ok(snapshot_at(&m, NOW).unwrap()),
                |_, _, _, _| panic!("invalid dispatched")
            )
            .is_err());
        }
        assert!(!path(&root.0, p["operation_id"].as_str().unwrap()).exists());
    }
    #[test]
    fn partial_glory_update_retains_durable_uncertainty_and_process_exit_can_resolve_it() {
        let root = Scratch::new();
        let m = Memory::fixture();
        let p = params(&m);
        let r = execute(
            &root.0,
            &p,
            || Ok(snapshot_at(&m, NOW).unwrap()),
            |s, f, c, v| {
                let mut w = Writer::new(&m);
                w.fail_at = Some(2);
                apply_at(&m, &mut w, s, f, c, v, NOW)
            },
        )
        .unwrap();
        assert_eq!(r["player_edit"]["state"], "uncertain");
        assert!(snapshot_at(&m, NOW).is_err());
        assert!(recover(&root.0, &p, || Err(fail("unreadable")), |_, _| Ok(false)).is_err());
        assert_eq!(
            status(&root.0, &p).unwrap()["player_edit"]["state"],
            "uncertain"
        );
        let expired = recover(
            &root.0,
            &p,
            || panic!("exited process read"),
            |_, _| Ok(true),
        )
        .unwrap();
        assert_eq!(expired["player_edit"]["state"], "expired");
    }
    #[test]
    fn response_loss_after_success_recovers_verified_without_rewriting() {
        let root = Scratch::new();
        let m = Memory::fixture();
        let p = params(&m);
        execute(
            &root.0,
            &p,
            || Ok(snapshot_at(&m, NOW).unwrap()),
            |s, f, c, v| {
                let mut w = Writer::new(&m);
                assert_eq!(
                    apply_at(&m, &mut w, s, f, c, v, NOW),
                    LiveEditOutcome::Verified
                );
                LiveEditOutcome::Uncertain("injected response loss".into())
            },
        )
        .unwrap();
        let r = recover(
            &root.0,
            &p,
            || Ok(snapshot_at(&m, NOW).unwrap()),
            |_, _| Ok(false),
        )
        .unwrap();
        assert_eq!(r["player_edit"]["state"], "verified");
    }
}
