//! Read-only Nioh 3 runtime adapter and `runtime.status` core.
//!
//! Scope: the migration's M3 read-only slice. The crate ports the shipped
//! Python runtime *read* surface and nothing else.
//!
//! - [`RuntimeOwnership`] is the ownership state machine behind
//!   `RuntimeApplication.status()`. The shipped host answers `runtime.status`
//!   without touching the game process, and so does this model.
//! - [`platform`] covers the `running_game_identity` contract: process
//!   discovery, the executable image, the fixed module version, the approved
//!   native runtime profile and its signature sites.
//! - [`ValidatedProcess`] carries the read-before-dereference rules of
//!   `native.NativeBatchOracle.open`: process creation identity, module-bounded
//!   ranges and captured-signature verification before any read is exposed.
//!
//! Boundaries, stated so a later worker cannot widen them by accident:
//!
//! - No writes. There is no `PROCESS_VM_WRITE`, no remote allocation, no
//!   thread creation, no debugger or Cheat Engine transport, and no launch,
//!   suspend, kill or close of the game or any user application.
//! - The only process access rights used are `PROCESS_QUERY_LIMITED_INFORMATION`
//!   for identity and `PROCESS_QUERY_INFORMATION | PROCESS_VM_READ` for reads.
//! - Every failure is a typed [`RuntimeError`]. A missing game reports
//!   [`RuntimeError::ProcessAbsent`]; it is never folded into a generic
//!   failure, and access denial is never reported as exit.
//! - Unknown versions, unapproved profiles, unresolved sites and mismatched
//!   signatures fail closed before an address is dereferenced.
//!
//! Platform functions are implemented for Windows. Other targets still build
//! and return [`RuntimeError::UnsupportedPlatform`], so the state and profile
//! models stay portable and unit-testable everywhere.

pub mod error;
pub mod mutation;
pub mod platform;
pub mod profile;
pub mod status;

pub use error::RuntimeError;
pub use platform::{
    discover_process_ids, file_version, identify_running_game, identify_running_game_for,
    identify_running_game_named, identify_running_game_named_for, module_range,
    process_creation_filetime, query_image_path, single_process_id, verify_game_executable,
    FileVersion, GameCompatibility, GameExecutableStatus, GameIdentity, ModuleRange,
    ProcessIdentity, ValidatedProcess, GAME_IMAGE_NAME, GAME_MODULE_NAME, IDENTITY_ACCESS,
    READER_ACCESS,
};
pub use profile::{
    default_pc_v2_00_02, load_research_profile, profile_for_game_version,
    supported_display_version, NativeRuntimeProfile, ProfileSite, PROFILE_SCHEMA,
    SIGNATURE_SITE_NAMES, SUPPORTED_GAME_VERSION, SUPPORTED_GAME_VERSIONS,
};
pub use status::{
    OverrideSession, RetiredOracle, RuntimeOwnership, RuntimeStatus, STATE_APPLIED_HIT,
    STATE_ARMED_NO_HIT, STATE_STOPPED, STATE_UNKNOWN, STATUS_METHOD,
};
