//! Windows runtime discovery, validated reads, and protected native operations.
//!
//! The read path validates process identity, module bounds and versioned
//! signatures before exposing memory. The mutation path owns a separate
//! session with versioned admission, explicit debugger and thread ownership,
//! cleanup, and readback through the protected runtime host.
//!
//! Unsupported platforms and unverifiable profiles return typed errors.

pub mod character;
pub mod combat_context;
pub mod compatibility_probe;
pub mod error;
pub mod inventory;
pub mod mutation;
pub mod platform;
pub mod profile;
pub mod status;

pub use error::RuntimeError;
pub use inventory::{
    inventory_sites, snapshot, FixtureMemory, InventoryMemory, InventoryProcess, InventoryRequest,
    ProcessInventoryMemory, DEFAULT_LIMIT, INVENTORY_EXECUTABLE_SHA256, INVENTORY_GAME_VERSION,
    MAX_LIMIT, MAX_START, SLOT_SAFETY_CEILING,
};
pub use platform::{
    discover_process_ids, file_sha256, file_version, identify_running_game,
    identify_running_game_for, identify_running_game_named, identify_running_game_named_for,
    module_range, process_creation_filetime, query_image_path, single_process_id,
    verify_game_executable, FileVersion, GameCompatibility, GameExecutableStatus, GameIdentity,
    ModuleRange, ProcessIdentity, ReadOnlyProcess, ValidatedProcess, GAME_IMAGE_NAME,
    GAME_MODULE_NAME, IDENTITY_ACCESS, READER_ACCESS,
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
