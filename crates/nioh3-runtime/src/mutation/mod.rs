//! Protected runtime mutation core (temporary overrides and count edits).
//!
//! This is a mutation slice. Unlike [`crate::platform`], which only ever
//! requests read rights, these modules request the minimum write rights for one
//! explicit operation and hold them only while that operation owns the target:
//!
//! | Operation | Rights | Shipped equivalent |
//! | --- | --- | --- |
//! | Override install/restore | `VM_OPERATION | VM_READ | VM_WRITE | QUERY_INFORMATION` | `runtime_auxiliary_override.PROCESS_ACCESS` |
//! | Count write | `QUERY_LIMITED_INFORMATION | VM_OPERATION | VM_WRITE` | `runtime_count_edit.WindowsCountMemory.write` |
//!
//! `PROCESS_ALL_ACCESS` is never requested, no thread is created or terminated,
//! and no memory is written before the identity, version, signature and range
//! gates have all passed. Everything is written against a small process
//! interface so a fault-injecting adapter can prove the restoration paths
//! without a game process; the Windows binding is the only code that touches a
//! real handle.

pub mod catalog;
pub mod count;
pub mod descriptor;
pub mod evidence;
pub mod inventory;
pub mod live_add;
pub mod live_batch;
pub mod memory;
pub mod native_abi;
#[cfg(any(windows, feature = "test-fake"))]
pub mod native_executor;
pub mod operations;
pub mod oracle;
pub mod session;
pub mod trampoline;
pub mod win_session;

#[cfg(test)]
mod count_tests;
#[cfg(any(test, feature = "test-fake"))]
mod fake;
#[cfg(any(test, feature = "test-fake"))]
pub mod live_fakes;
#[cfg(test)]
mod live_tests;
#[cfg(test)]
mod native_executor_tests;
#[cfg(any(test, feature = "test-fake"))]
pub mod native_fakes;
#[cfg(test)]
mod session_tests;

pub use count::{
    canonical_json, stable_identity, CountEditor, CountLayout, CountMemory, CountPlan,
    CountProcesses, CountState, CountStatus, TargetCapture, WindowsCountMemory,
    PC_V201_COUNT_LAYOUT,
};
pub use descriptor::{
    assembly_descriptor, new_assembly_record, verify_assembly_preview, ASSEMBLY_FLAGS,
    DESCRIPTOR_SIZE,
};
pub use evidence::{defined, verify, verify_dispatch, verify_persistence, REGISTERS};
pub use inventory::{
    capture_index, capture_inventory, capture_read_only, index_entries, inventory_entries,
    inventory_json, resolve_inventory_pointers, Inventory, InventoryEntry, InventoryGlobalMode,
    InventoryLayout, InventoryPointers, InventoryProcess, NativeIndex, INSERTION_SIGNATURE,
    INVENTORY_GLOBAL_MODE_DIRECT_DATA, INVENTORY_GLOBAL_MODE_MANAGER_OBJECT,
    PC_V201_INVENTORY_LAYOUT,
};
pub use live_add::{
    saved_scroll_records, CandidateEffect, CandidateStage, CatalogPolicy, InstallationCandidate,
    LiveAddApplication, LiveAddExecutor, PreparedLiveAdd, SaveBackup, SaveCheckpoint,
    LIVE_ADD_DISPLAY_VERSION, SCROLL_GROUP_OFFSET, SCROLL_SLOT_COUNT,
};
pub use live_batch::LiveAddBatch;
pub use operations::{LiveAddOperations, OperationSnapshot, OperationState};

#[cfg(windows)]
pub use count::{WindowsCountMemoryAdapter, WindowsCountProcesses};
pub use memory::{TargetProcess, COUNT_WRITE_ACCESS, OVERRIDE_ACCESS, READ_ACCESS};
pub use session::{
    ChallengeOverrideProfile, OverrideGroup, OverrideSession, RuntimeMutationHost, SessionMemory,
    SessionSite, CAPACITY_RVA, CAPACITY_SIGNATURE, CHALLENGE_DISPLAY_VERSION, COUNTER_RESERVE,
    REMOTE_ALLOCATION_SIZE,
};
pub use trampoline::{
    build_challenge_trampoline, build_override_trampoline, build_relative_jump, EnemyGroup,
    OverrideProfile, MAX_ENEMY_GROUPS, TRAMPOLINE_CAPACITY,
};

#[cfg(windows)]
pub use memory::WindowsProcess;

#[cfg(windows)]
pub use session::{WindowsMutationHost, WindowsOverrideSession, WindowsSessionMemory};

/// The fault-injecting adapter, exported only for tests and for the
/// off-by-default `test-fake` build the cross-language gate uses.
#[cfg(any(test, feature = "test-fake"))]
pub use fake::{FakeCountMemory, FakeMemoryFactory, Faults, FAKE_MODULE_BASE};
