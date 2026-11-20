//! Explicit debug-only file-backed UI acceptance. Never enabled by default.
use crate::error::HostError;
use nioh3_runtime::combat_context::SnapshotOutcome;
use std::path::PathBuf;

pub fn enabled() -> bool {
    std::env::var("NIOH3_E2E_NO_GAME").as_deref() == Ok("1")
}

pub fn first_use_memory() -> Result<Option<nioh3_runtime::FixtureMemory>, HostError> {
    let Some(path) = std::env::var_os("NIOH3_E2E_FIRST_USE_MEMORY") else {
        return Ok(None);
    };
    let root = PathBuf::from(
        std::env::var_os("NIOH3_E2E_CAPTURE_ROOT")
            .ok_or_else(|| HostError::rejected("OFFLINE_E2E_CAPTURE_ROOT_REQUIRED"))?,
    )
    .canonicalize()
    .map_err(|e| HostError::rejected(e.to_string()))?;
    let path = PathBuf::from(path)
        .canonicalize()
        .map_err(|e| HostError::rejected(e.to_string()))?;
    if !path.starts_with(root) {
        return Err(HostError::rejected(
            "OFFLINE_E2E_CAPSULE_OUTSIDE_OWNED_ROOT",
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| HostError::rejected(e.to_string()))?;
    let json = serde_json::from_slice(&bytes).map_err(|e| HostError::rejected(e.to_string()))?;
    nioh3_runtime::FixtureMemory::from_json(&json)
        .map(Some)
        .map_err(HostError::from_runtime)
}

pub fn snapshot() -> Result<Option<SnapshotOutcome>, HostError> {
    let root = PathBuf::from(
        std::env::var_os("NIOH3_E2E_CAPTURE_ROOT")
            .ok_or_else(|| HostError::rejected("OFFLINE_E2E_CAPTURE_ROOT_REQUIRED"))?,
    )
    .canonicalize()
    .map_err(|error| HostError::rejected(error.to_string()))?;
    let memory = std::env::var_os("NIOH3_E2E_SOURCE_MEMORY");
    let path = PathBuf::from(
        memory
            .clone()
            .or_else(|| std::env::var_os("NIOH3_E2E_SOURCE_CAPSULE"))
            .ok_or_else(|| HostError::rejected("OFFLINE_E2E_CAPSULE_REQUIRED"))?,
    )
    .canonicalize()
    .map_err(|error| HostError::rejected(error.to_string()))?;
    if !path.starts_with(root) {
        return Err(HostError::rejected(
            "OFFLINE_E2E_CAPSULE_OUTSIDE_OWNED_ROOT",
        ));
    }
    if memory.is_some() {
        let raw = std::fs::read(&path).map_err(|error| HostError::rejected(error.to_string()))?;
        let json =
            serde_json::from_slice(&raw).map_err(|error| HostError::rejected(error.to_string()))?;
        let fixture = nioh3_runtime::FixtureMemory::from_json(&json)
            .map_err(|error| HostError::rejected(error.to_string()))?;
        return nioh3_runtime::combat_context::read_combat_snapshot(&fixture)
            .map(Some)
            .map_err(|error| HostError::rejected(error.to_string()));
    }
    crate::loadout_observation::snapshot_for_offline_e2e(&path)
        .map(|snapshot| Some(SnapshotOutcome::Ready(Box::new(snapshot))))
}

pub fn origin() -> crate::loadout_observation::ObservationOrigin {
    if std::env::var_os("NIOH3_E2E_SOURCE_MEMORY").is_some() {
        crate::loadout_observation::ObservationOrigin::ControlledRetainedInput
    } else {
        crate::loadout_observation::ObservationOrigin::RetainedCaptureReplay
    }
}
pub(crate) fn epoch_receipt() -> Result<PathBuf, HostError> {
    let directory = PathBuf::from(
        std::env::var_os("NIOH3_E2E_CAPTURE_ROOT")
            .ok_or_else(|| HostError::rejected("OFFLINE_E2E_CAPTURE_ROOT_REQUIRED"))?,
    )
    .canonicalize()
    .map_err(|error| HostError::rejected(error.to_string()))?;
    let receipt = PathBuf::from(
        std::env::var_os("NIOH3_E2E_SOURCE_EPOCH")
            .ok_or_else(|| HostError::rejected("OFFLINE_E2E_SOURCE_EPOCH_REQUIRED"))?,
    )
    .canonicalize()
    .map_err(|error| HostError::rejected(error.to_string()))?;
    if !receipt.starts_with(&directory) {
        return Err(HostError::rejected(
            "OFFLINE_E2E_CAPSULE_OUTSIDE_OWNED_ROOT",
        ));
    }
    Ok(receipt)
}
