//! Shared durable receipts for narrowly scoped player edits.
use crate::HostError;
use serde_json::Value;
use std::io::{Read, Write};
use std::path::Path;
fn fail(detail: impl Into<String>) -> HostError {
    HostError::coded("PLAYER_EDIT_REFUSED", detail)
}
pub(crate) fn assert_no_pending(root: &Path) -> Result<(), HostError> {
    for namespace in ["prestige-points", "player-resources"] {
        let directory = root.join(namespace);
        if !directory.try_exists().map_err(|e| fail(e.to_string()))? {
            continue;
        }
        for (count, file) in std::fs::read_dir(directory)
            .map_err(|e| fail(e.to_string()))?
            .enumerate()
        {
            if count >= 8192 {
                return Err(fail("Player edit history exceeds bounds"));
            }
            let path = file.map_err(|e| fail(e.to_string()))?.path();
            if path.extension().is_some_and(|e| e == "json") && load(&path)?["state"] == "uncertain"
            {
                return Err(fail("Check the previous player edit before another write"));
            }
        }
    }
    Ok(())
}
pub(crate) fn operation_id(params: &Value) -> Result<&str, HostError> {
    let id = params["operation_id"]
        .as_str()
        .ok_or_else(HostError::invalid_request)?;
    if id.len() != 36
        || !id.bytes().enumerate().all(|(i, b)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit() && !b.is_ascii_uppercase()
            }
        })
    {
        return Err(HostError::invalid_request());
    }
    Ok(id)
}
pub(crate) fn load(path: &Path) -> Result<Value, HostError> {
    let file = std::fs::File::open(path).map_err(|e| fail(e.to_string()))?;
    let mut data = Vec::new();
    file.take(65537)
        .read_to_end(&mut data)
        .map_err(|e| fail(e.to_string()))?;
    if data.len() > 65536 {
        return Err(fail("Oversized player edit receipt"));
    }
    let value: Value = serde_json::from_slice(&data).map_err(|e| fail(e.to_string()))?;
    if !matches!(
        value["state"].as_str(),
        Some("uncertain" | "verified" | "rejected" | "not_applied" | "expired")
    ) || value["operation_id"].as_str().is_none()
        || value["request_digest"].as_str().is_none()
    {
        return Err(fail("Invalid player edit receipt"));
    }
    Ok(value)
}
pub(crate) fn store(root: &Path, namespace: &str, value: &Value) -> Result<(), HostError> {
    let id = operation_id(value)?;
    let directory = root.join(namespace);
    std::fs::create_dir_all(&directory).map_err(|e| fail(e.to_string()))?;
    let target = directory.join(format!("{id}.json"));
    let temporary = target.with_extension("tmp");
    // The exclusive admission lock covers every transition. A crash-left temp
    // is replaceable: only a synced/renamed JSON receipt admits a game write.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temporary)
        .map_err(|e| fail(e.to_string()))?;
    file.write_all(&serde_json::to_vec_pretty(value).map_err(|e| fail(e.to_string()))?)
        .and_then(|_| file.sync_all())
        .map_err(|e| fail(e.to_string()))?;
    std::fs::rename(temporary, target).map_err(|e| fail(e.to_string()))?;
    Ok(())
}
