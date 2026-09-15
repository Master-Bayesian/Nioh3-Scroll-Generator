//! Typed, fail-closed runtime adapter errors.

use std::fmt;

/// Every failure the adapter can produce has its own variant, so a caller can
/// always separate "no game is running" from "the game could not be inspected"
/// and both from "the profile or signature was rejected".
///
/// Messages mirror the shipped Python text where that text is already an
/// English contract (`native.load_native_runtime_profile`,
/// `native.native_runtime_profile_for_game_version`,
/// `runtime_application.running_game_identity`). The one Chinese string in
/// `native.find_nioh3_pid` is reported as English and its machine code, not its
/// text, is the stable identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    /// The crate was built for a non-Windows target.
    UnsupportedPlatform,
    /// No running process matched the image name. Not a failure.
    ProcessAbsent { image: String },
    /// More than one process matched the image name; ownership is ambiguous.
    AmbiguousProcess { image: String, count: usize },
    /// `OpenProcess` refused the requested access.
    OpenProcess { pid: u32, code: u32 },
    /// `QueryFullProcessImageNameW` failed.
    QueryImageName { pid: u32, code: u32 },
    /// A read-only process query (`GetExitCodeProcess`/`GetProcessTimes`) failed.
    ProcessQuery {
        pid: u32,
        code: u32,
        detail: &'static str,
    },
    /// The pid no longer refers to a live process (exited, or gone entirely).
    ProcessGone { pid: u32 },
    /// The pid refers to a different process instance than the expected one.
    ProcessInstanceChanged { pid: u32 },
    /// The requested module is not loaded in the process.
    ModuleNotFound { pid: u32, module: String },
    /// The module snapshot could not be taken.
    ModuleSnapshot { pid: u32, code: u32 },
    /// `ReadProcessMemory` failed.
    MemoryRead {
        address: u64,
        size: usize,
        code: u32,
    },
    /// `ReadProcessMemory` returned fewer bytes than requested.
    ShortRead {
        address: u64,
        expected: usize,
        actual: usize,
    },
    /// A site or read range does not fit inside the validated module image.
    RangeOutOfBounds { offset: u64, size: u64, limit: u64 },
    /// `WriteProcessMemory` failed.
    MemoryWrite {
        address: u64,
        size: usize,
        code: u32,
    },
    /// `WriteProcessMemory` returned fewer bytes than requested.
    ShortWrite {
        address: u64,
        expected: usize,
        actual: usize,
    },
    /// `VirtualProtectEx` failed.
    MemoryProtect {
        address: u64,
        size: usize,
        code: u32,
    },
    /// `FlushInstructionCache` failed after an executable write.
    InstructionCacheFlush {
        address: u64,
        size: usize,
        code: u32,
    },
    /// No free region inside the rel32 range of the hook could be committed.
    AllocationUnavailable { address: u64, size: u64 },
    /// `VirtualFreeEx` failed.
    AllocationRelease { address: u64, code: u32 },
    /// A `rel32` jump between the hook and the trampoline is out of range.
    TrampolineOutOfRange { source: u64, target: u64 },
    /// The generated trampoline does not fit its allocation.
    TrampolineTooLarge { size: u64, capacity: u64 },
    /// A mutation profile or session argument is not supported.
    InvalidOverrideProfile { detail: String },
    /// The session owns no process handle; it never started or was released.
    SessionNotOpen,
    /// The hook bytes belong to another writer; nothing is overwritten or freed.
    HookModified { address: u64 },
    /// A restore could not be confirmed and the process did not positively exit.
    HookRestoreUnverified { detail: String },
    /// The restore finished without a positive confirmation.
    HookNotRestored { address: u64 },
    /// Another protected operation still owns the runtime.
    RuntimeBusy,
    /// The requested remaining count is outside `0..=7`.
    InvalidCount { value: i64 },
    /// The save, record or instance changed since the plan was prepared.
    CountSourceChanged { detail: String },
    /// The instance is no longer the one the plan captured.
    CountInstanceChanged,
    /// The serial is not present in the current inventory.
    CountInstanceUnavailable { serial: u64 },
    /// The read-only inventory capture refused the game state.
    ///
    /// Message mirrors `live_inventory.capture_inventory` text exactly.
    InventoryInvalid { detail: String },
    /// The candidate transfer failed its context, identity or stage gate.
    ///
    /// Message mirrors `candidate_transfer.import_candidate`,
    /// `search_application.require_search_candidate_ready` and
    /// `core_services.OperationPolicy` text exactly.
    CandidateRejected { detail: String },
    /// A live-add preparation or execution gate refused the reviewed plan.
    ///
    /// Message mirrors `live_add_application.LiveAddApplication` text exactly.
    LiveAddRejected { detail: String },
    /// Independent verification of a native insertion failed.
    ///
    /// Message mirrors `live_add_evidence.verify` /
    /// `live_add_descriptor.verify_assembly_preview` text exactly.
    LiveAddVerification { detail: String },
    /// A dispatched insertion is not acknowledged; it may never be replayed.
    LiveAddUncertain { operation_id: String },
    /// The selected batch itself is invalid, before any native work starts.
    BatchRejected { detail: String },
    /// The automatic backup does not match the planned source digest.
    BackupMismatch { path: String },
    /// A receipt, claim or plan does not match the one on record.
    ReceiptConflict { detail: String },
    /// Operation identifiers could not be generated.
    EntropyUnavailable,
    /// The fixed file version resource could not be read.
    FileVersionUnreadable { path: String, code: u32 },
    /// A running game executable exists but is not a verified supported version.
    GameExecutableUnsupported { path: String, state: &'static str },
    /// The executable's fixed file version is outside the verified range.
    UnsupportedGameVersion { display: String },
    /// The version's runtime profile is not approved for product use.
    ProfileNotApproved { profile: String },
    /// The profile document uses an unsupported schema.
    ProfileSchema { schema: String },
    /// A profile site has no resolved RVA.
    ProfileUnresolved { site: String },
    /// A profile site has no captured signature.
    ProfileMissingSignature { site: String },
    /// The profile document is unreadable, not JSON, or malformed.
    ProfileIntegrity { detail: String },
    /// A captured signature did not match the running module image.
    SignatureMismatch { site: String, rva: u64 },
    /// A remote machine-code or ABI shape refusal. The shipped emitters raise
    /// `ValueError`/`AssertionError` for exactly these cases.
    NativeAbi { detail: String },
    /// The native live-add dispatch adapter refused or failed one operation.
    NativeDispatch { detail: String },
    /// The batch oracle refused one request or is not usable.
    OracleRejected { detail: String },
    /// The batch oracle still owes a retired remote call.
    OracleBusy,
    /// Filesystem access failed.
    Io { path: String, detail: String },
}

impl RuntimeError {
    /// Stable machine code for reports, tests and the future worker boundary.
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "UNSUPPORTED_PLATFORM",
            Self::ProcessAbsent { .. } => "PROCESS_ABSENT",
            Self::AmbiguousProcess { .. } => "AMBIGUOUS_PROCESS",
            Self::OpenProcess { .. } => "OPEN_PROCESS_FAILED",
            Self::QueryImageName { .. } => "QUERY_IMAGE_NAME_FAILED",
            Self::ProcessQuery { .. } => "PROCESS_QUERY_FAILED",
            Self::ProcessGone { .. } => "PROCESS_GONE",
            Self::ProcessInstanceChanged { .. } => "PROCESS_INSTANCE_CHANGED",
            Self::ModuleNotFound { .. } => "MODULE_NOT_FOUND",
            Self::ModuleSnapshot { .. } => "MODULE_SNAPSHOT_FAILED",
            Self::MemoryRead { .. } => "MEMORY_READ_FAILED",
            Self::ShortRead { .. } => "SHORT_READ",
            Self::RangeOutOfBounds { .. } => "RANGE_OUT_OF_BOUNDS",
            Self::MemoryWrite { .. } => "MEMORY_WRITE_FAILED",
            Self::ShortWrite { .. } => "SHORT_WRITE",
            Self::MemoryProtect { .. } => "MEMORY_PROTECT_FAILED",
            Self::InstructionCacheFlush { .. } => "INSTRUCTION_CACHE_FLUSH_FAILED",
            Self::AllocationUnavailable { .. } => "ALLOCATION_UNAVAILABLE",
            Self::AllocationRelease { .. } => "ALLOCATION_RELEASE_FAILED",
            Self::TrampolineOutOfRange { .. } => "TRAMPOLINE_OUT_OF_RANGE",
            Self::TrampolineTooLarge { .. } => "TRAMPOLINE_TOO_LARGE",
            Self::InvalidOverrideProfile { .. } => "INVALID_OVERRIDE_PROFILE",
            Self::SessionNotOpen => "SESSION_NOT_OPEN",
            Self::HookModified { .. } => "HOOK_MODIFIED",
            Self::HookRestoreUnverified { .. } => "HOOK_RESTORE_UNVERIFIED",
            Self::HookNotRestored { .. } => "HOOK_NOT_RESTORED",
            Self::RuntimeBusy => "RUNTIME_BUSY",
            Self::InvalidCount { .. } => "INVALID_COUNT",
            Self::CountSourceChanged { .. } => "COUNT_SOURCE_CHANGED",
            Self::CountInstanceChanged => "COUNT_INSTANCE_CHANGED",
            Self::CountInstanceUnavailable { .. } => "COUNT_INSTANCE_UNAVAILABLE",
            Self::InventoryInvalid { .. } => "INVENTORY_INVALID",
            Self::CandidateRejected { .. } => "CANDIDATE_REJECTED",
            Self::LiveAddRejected { .. } => "LIVE_ADD_REJECTED",
            Self::LiveAddVerification { .. } => "LIVE_ADD_VERIFICATION",
            Self::LiveAddUncertain { .. } => "LIVE_ADD_UNCERTAIN",
            Self::BatchRejected { .. } => "BATCH_REJECTED",
            Self::BackupMismatch { .. } => "BACKUP_MISMATCH",
            Self::ReceiptConflict { .. } => "RECEIPT_CONFLICT",
            Self::EntropyUnavailable => "ENTROPY_UNAVAILABLE",
            Self::FileVersionUnreadable { .. } => "FILE_VERSION_UNREADABLE",
            Self::GameExecutableUnsupported { .. } => "GAME_EXECUTABLE_UNSUPPORTED",
            Self::UnsupportedGameVersion { .. } => "UNSUPPORTED_GAME_VERSION",
            Self::ProfileNotApproved { .. } => "PROFILE_NOT_APPROVED",
            Self::ProfileSchema { .. } => "PROFILE_SCHEMA",
            Self::ProfileUnresolved { .. } => "PROFILE_UNRESOLVED",
            Self::ProfileMissingSignature { .. } => "PROFILE_MISSING_SIGNATURE",
            Self::ProfileIntegrity { .. } => "PROFILE_INTEGRITY",
            Self::SignatureMismatch { .. } => "SIGNATURE_MISMATCH",
            Self::NativeAbi { .. } => "NATIVE_ABI",
            Self::NativeDispatch { .. } => "NATIVE_DISPATCH",
            Self::OracleRejected { .. } => "ORACLE_REJECTED",
            Self::OracleBusy => "ORACLE_BUSY",
            Self::Io { .. } => "IO",
        }
    }

    /// English operator-facing text. Stable messages are shared with Python.
    pub fn message(&self) -> String {
        match self {
            Self::UnsupportedPlatform => "the runtime adapter requires Windows".to_string(),
            Self::ProcessAbsent { image } => {
                format!("no running process matches {image}")
            }
            Self::AmbiguousProcess { image, count } => {
                format!("exactly one {image} must be running, but {count} were found")
            }
            Self::OpenProcess { pid, code } => {
                format!("OpenProcess({pid}) failed with error {code}")
            }
            Self::QueryImageName { pid, code } => {
                format!("QueryFullProcessImageNameW({pid}) failed with error {code}")
            }
            Self::ProcessQuery { pid, code, detail } => {
                format!("{detail}({pid}) failed with error {code}")
            }
            Self::ProcessGone { pid } => {
                format!("process {pid} is no longer running")
            }
            Self::ProcessInstanceChanged { pid } => {
                format!("process {pid} was replaced by a different process instance")
            }
            Self::ModuleNotFound { pid, module } => {
                format!("module {module} was not found in process {pid}")
            }
            Self::ModuleSnapshot { pid, code } => {
                format!("CreateToolhelp32Snapshot(modules, {pid}) failed with error {code}")
            }
            Self::MemoryRead {
                address,
                size,
                code,
            } => format!("ReadProcessMemory({address:#x}, {size:#x}) failed with error {code}"),
            Self::ShortRead {
                address,
                expected,
                actual,
            } => {
                format!("ReadProcessMemory({address:#x}, {expected:#x}) returned {actual:#x} bytes")
            }
            Self::RangeOutOfBounds {
                offset,
                size,
                limit,
            } => format!(
                "range {offset:#x}+{size:#x} leaves the validated image of {limit:#x} bytes"
            ),
            Self::MemoryWrite { address, size, code } => format!(
                "WriteProcessMemory({address:#x}, {size:#x}) failed with error {code}"
            ),
            Self::ShortWrite {
                address,
                expected,
                actual,
            } => format!(
                "WriteProcessMemory({address:#x}, {expected:#x}) wrote {actual:#x} bytes"
            ),
            Self::MemoryProtect { address, size, code } => format!(
                "VirtualProtectEx({address:#x}, {size:#x}) failed with error {code}"
            ),
            Self::InstructionCacheFlush { address, size, code } => format!(
                "FlushInstructionCache({address:#x}, {size:#x}) failed with error {code}"
            ),
            Self::AllocationUnavailable { address, size } => format!(
                "no {size:#x}-byte trampoline can be allocated within jump range of {address:#x}"
            ),
            Self::AllocationRelease { address, code } => {
                format!("VirtualFreeEx({address:#x}) failed with error {code}")
            }
            Self::TrampolineOutOfRange { source, target } => format!(
                "trampoline at {target:#x} is outside the x64 rel32 jump range of {source:#x}"
            ),
            Self::TrampolineTooLarge { size, capacity } => {
                format!("runtime override trampoline of {size:#x} exceeds {capacity:#x} bytes")
            }
            Self::InvalidOverrideProfile { detail } => detail.clone(),
            Self::SessionNotOpen => "override session is not open".to_string(),
            Self::HookModified { address } => format!(
                "the hook at {address:#x} was changed by another program; it will not be overwritten"
            ),
            Self::HookRestoreUnverified { detail } => format!(
                "the temporary override could not be read or restored and the process has not exited ({detail})"
            ),
            Self::HookNotRestored { address } => {
                format!("the temporary override at {address:#x} was not removed")
            }
            Self::RuntimeBusy => {
                "stop the existing override and wait for pending native calls".to_string()
            }
            Self::InvalidCount { value } => {
                format!("Remaining count must be an integer from 0 to 7, not {value}")
            }
            Self::CountSourceChanged { detail } => detail.clone(),
            Self::CountInstanceChanged => "count recovery process or instance differs".to_string(),
            Self::CountInstanceUnavailable { serial } => {
                format!("Scroll instance {serial} is no longer in the current inventory")
            }
            Self::InventoryInvalid { detail } => detail.clone(),
            Self::CandidateRejected { detail } => detail.clone(),
            Self::LiveAddRejected { detail } => detail.clone(),
            Self::LiveAddVerification { detail } => detail.clone(),
            Self::LiveAddUncertain { operation_id } => format!(
                "Uncertain insertion {operation_id}; recover its receipt before preparing another"
            ),
            Self::BatchRejected { detail } => detail.clone(),
            Self::BackupMismatch { path } => {
                format!("Automatic backup changed; retain uncertain receipt ({path})")
            }
            Self::ReceiptConflict { detail } => detail.clone(),
            Self::EntropyUnavailable => "operation identifiers could not be generated".to_string(),
            Self::FileVersionUnreadable { path, code } => {
                format!("fixed file version of {path} is unreadable ({code})")
            }
            Self::GameExecutableUnsupported { path, state } => format!(
                "running game executable {path} is not a verified supported version ({state})"
            ),
            Self::UnsupportedGameVersion { display } => {
                format!("unsupported Nioh 3 executable version: {display}")
            }
            Self::ProfileNotApproved { profile } => {
                format!("{profile} runtime profile is not approved for product use")
            }
            // Text kept identical to `load_native_runtime_profile`; the observed
            // schema stays available on the variant.
            Self::ProfileSchema { .. } => "unsupported native runtime profile schema".to_string(),
            Self::ProfileUnresolved { site } => {
                format!("native runtime profile site {site} is unresolved")
            }
            Self::ProfileMissingSignature { site } => {
                format!("native runtime profile site {site} has no captured signature")
            }
            Self::ProfileIntegrity { detail } => detail.clone(),
            Self::SignatureMismatch { site, rva } => {
                format!("native signature {site} at {rva:#x} does not match the running game image")
            }
            Self::NativeAbi { detail } => detail.clone(),
            Self::NativeDispatch { detail } => detail.clone(),
            Self::OracleRejected { detail } => detail.clone(),
            Self::OracleBusy => {
                "the batch oracle still owes a retired native call".to_string()
            }
            Self::Io { path, detail } => format!("{path}: {detail}"),
        }
    }

    /// The runtime profile document could not be used at all.
    pub fn is_profile_rejection(&self) -> bool {
        matches!(
            self,
            Self::ProfileNotApproved { .. }
                | Self::ProfileSchema { .. }
                | Self::ProfileUnresolved { .. }
                | Self::ProfileMissingSignature { .. }
                | Self::ProfileIntegrity { .. }
                | Self::UnsupportedGameVersion { .. }
        )
    }

    /// The game could not be located; this is not an inspection failure.
    pub fn is_absence(&self) -> bool {
        matches!(self, Self::ProcessAbsent { .. })
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message())
    }
}

impl std::error::Error for RuntimeError {}

#[cfg(test)]
mod tests {
    use super::RuntimeError;

    #[test]
    fn absence_is_distinct_from_inspection_failure() {
        let absent = RuntimeError::ProcessAbsent {
            image: "Nioh3.exe".to_string(),
        };
        let denial = RuntimeError::OpenProcess { pid: 4, code: 5 };
        assert!(absent.is_absence());
        assert!(!denial.is_absence());
        assert_ne!(absent.code(), denial.code());
    }

    #[test]
    fn profile_rejections_share_one_predicate() {
        let cases = [
            RuntimeError::ProfileSchema {
                schema: "other".to_string(),
            },
            RuntimeError::ProfileUnresolved {
                site: "canonicalize".to_string(),
            },
            RuntimeError::ProfileMissingSignature {
                site: "init_compact".to_string(),
            },
            RuntimeError::ProfileNotApproved {
                profile: "pc_v2_01".to_string(),
            },
        ];
        for case in cases {
            assert!(case.is_profile_rejection(), "{case:?}");
            assert!(!case.is_absence());
        }
        assert!(!RuntimeError::SignatureMismatch {
            site: "init_compact".to_string(),
            rva: 0x1000,
        }
        .is_profile_rejection());
    }
}
