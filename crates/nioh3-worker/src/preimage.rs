//! Typed ABI bindings for the shipped `nioh3_effect_preimage_accelerator.dll`.
//!
//! Mirrors `nioh3_scroll_editor/effect_preimage_accelerator.py`. Unlike the seed
//! accelerator this library exports no ABI number and no build id, so the
//! module pins the verified artifact instead: the file must hash to
//! [`PREIMAGE_SHA256`] and must expose all four documented exports before a
//! single call is made. A stale, substituted or unreadable library fails closed
//! with a named error; no code path silently falls back to a different scanner.
//!
//! The library is deliberately never unloaded, matching the Python module's
//! cached `ctypes.WinDLL`, so no in-flight call can race a library unload. All
//! `unsafe` is contained in the private [`platform`] module and the thin
//! wrappers that call it.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use sha2::{Digest, Sha256};

/// File name the product loads from `<application root>/bin`.
pub const PREIMAGE_LIBRARY: &str = "nioh3_effect_preimage_accelerator.dll";
/// SHA-256 of the verified shipped artifact (232,448 bytes).
pub const PREIMAGE_SHA256: &str =
    "28d027b526eaa758025e52d8586a8ecdcaa575a3130e1aab46ba7e65b5a9eeac";
/// `ERROR_RESULT`: the collecting export reports failure with this count.
pub const ERROR_RESULT: u64 = u64::MAX;
/// `-2`: the forward matcher reports a missing DirectCompute backend.
pub const STATUS_UNAVAILABLE: i32 = -2;
/// Draw constraints one packed path descriptor can carry (`constraints[6]`).
pub const PATH_CONSTRAINT_LIMIT: usize = 6;
/// Smallest output capacity the Python adapter accepts.
pub const MIN_OUTPUT_CAPACITY: usize = 1;
/// Largest output capacity the Python adapter accepts.
pub const MAX_OUTPUT_CAPACITY: usize = 1_000_000;
/// Result capacity the shipped search layer reserves per chunk.
pub const DEFAULT_OUTPUT_CAPACITY: usize = 100_000;
/// Largest window one preimage sweep scans (`chunk_trials` in the shipped page).
pub const MAX_PREIMAGE_TRIALS: u64 = 256_000_000;
/// Largest Seed batch one forward-filter call accepts.
pub const MAX_PREDICATE_BATCH: usize = 1_000_000;
/// Largest candidate table one forward-filter call accepts.
pub const MAX_CANDIDATES: usize = 4096;
/// Largest criterion group count one forward-filter call accepts.
pub const MAX_CRITERION_GROUPS: usize = 32;
/// Category capacity slots the forward matcher reads.
pub const CATEGORY_CAPACITY_SLOTS: usize = 32;

/// `AMD_VENDOR_ID`.
pub const AMD_VENDOR_ID: u32 = 0x1002;
/// `NVIDIA_VENDOR_ID`.
pub const NVIDIA_VENDOR_ID: u32 = 0x10DE;
/// `INTEL_VENDOR_ID`.
pub const INTEL_VENDOR_ID: u32 = 0x8086;

/// Serializes every call into the module.
///
/// The library keeps process-global diagnostics and scratch buffers, so two
/// concurrent sweeps would race them. Lock order everywhere is: seed-accelerator
/// call lock first, then this lock, so a page that filters its matches with the
/// seed layer can never deadlock against one that does not.
static PREIMAGE_CALL_LOCK: Mutex<()> = Mutex::new(());

/// One `PathConstraintInput` (16 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PathConstraintInput {
    pub draw_index: u32,
    pub start_u16: u32,
    pub end_u16: u32,
    pub reserved: u32,
}

/// One `EffectPathInput` (112 bytes): up to six draw constraints.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectPathInput {
    pub promoted_slot: i32,
    pub constraint_count: u32,
    pub reserved0: u32,
    pub reserved1: u32,
    pub constraints: [PathConstraintInput; PATH_CONSTRAINT_LIMIT],
}

impl Default for EffectPathInput {
    fn default() -> Self {
        Self {
            promoted_slot: -1,
            constraint_count: 0,
            reserved0: 0,
            reserved1: 0,
            constraints: [PathConstraintInput::default(); PATH_CONSTRAINT_LIMIT],
        }
    }
}

/// One `EffectCandidateInput` (44 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EffectCandidateInput {
    pub effect_id: u32,
    pub group_key: u32,
    pub category_key: u32,
    pub conflict_mask_0: u32,
    pub conflict_mask_1: u32,
    pub normal_weight: u32,
    pub promoted_weight: u32,
    pub final_weight_common: u32,
    pub final_weight_special: u32,
    pub completion_candidate: u32,
    pub value_one_roll_mask: u32,
}

/// One `SpecialGroupInput` (16 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpecialGroupInput {
    pub group_key: u32,
    pub conflict_mask_0: u32,
    pub conflict_mask_1: u32,
    pub effect_id: u32,
}

const _: () = {
    assert!(std::mem::size_of::<PathConstraintInput>() == 16);
    assert!(std::mem::size_of::<EffectPathInput>() == 112);
    assert!(std::mem::size_of::<EffectCandidateInput>() == 44);
    assert!(std::mem::size_of::<SpecialGroupInput>() == 16);
};

/// The execution policy one job pins for the effect-preimage routes.
///
/// The shipped policy is strict hardware by default: the route must never be
/// answered by a slow replay that the user did not opt into, and never by a
/// fabricated answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreimagePolicy {
    /// Hardware only; no usable device is a hard failure.
    StrictGpu,
    /// The caller explicitly accepted a non-accelerated path.
    AllowCpuFallback,
}

impl PreimagePolicy {
    /// The policy a search request asks for, from `allow_cpu_fallback`.
    pub fn from_allow_cpu_fallback(allow_cpu_fallback: bool) -> Self {
        if allow_cpu_fallback {
            PreimagePolicy::AllowCpuFallback
        } else {
            PreimagePolicy::StrictGpu
        }
    }

    /// The stored encoding of this policy.
    pub fn raw(self) -> u8 {
        match self {
            PreimagePolicy::StrictGpu => 0,
            PreimagePolicy::AllowCpuFallback => 1,
        }
    }

    /// Decode a stored policy; anything unknown pins strict hardware.
    pub fn from_raw(raw: u8) -> Self {
        match raw {
            1 => PreimagePolicy::AllowCpuFallback,
            _ => PreimagePolicy::StrictGpu,
        }
    }
}

/// Which DirectCompute vendor served the last call (`last_effect_preimage_backend`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreimageBackend {
    /// No call has run on this accelerator yet.
    NotUsed,
    /// `d3d11_amd`.
    Amd,
    /// `d3d11_nvidia`.
    Nvidia,
    /// `d3d11_intel`.
    Intel,
    /// `d3d11_other`, with the reported vendor id.
    Other(u32),
}

impl PreimageBackend {
    /// Decode the vendor id the native call wrote back.
    pub fn from_vendor(vendor_id: u32) -> Self {
        match vendor_id {
            AMD_VENDOR_ID => PreimageBackend::Amd,
            NVIDIA_VENDOR_ID => PreimageBackend::Nvidia,
            INTEL_VENDOR_ID => PreimageBackend::Intel,
            other => PreimageBackend::Other(other),
        }
    }

    /// The shipped `last_effect_preimage_backend()` string.
    pub fn as_str(self) -> &'static str {
        match self {
            PreimageBackend::NotUsed => "not_used",
            PreimageBackend::Amd => "d3d11_amd",
            PreimageBackend::Nvidia => "d3d11_nvidia",
            PreimageBackend::Intel => "d3d11_intel",
            PreimageBackend::Other(_) => "d3d11_other",
        }
    }
}

/// The adapter the accelerator selected (`d3d11_effect_adapter_info`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreimageAdapterInfo {
    pub description: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub dedicated_video_memory: u64,
    pub shared_system_memory: u64,
}

/// The pinned identity of the loaded artifact.
///
/// This library exports no build id, so the identity is the verified content
/// hash plus the export set that actually resolved. Nothing here is invented
/// from a version string the artifact does not publish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreimageIdentity {
    pub path: PathBuf,
    pub sha256: String,
    pub exports: Vec<&'static str>,
}

/// Why the effect-preimage library is unusable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreimageError {
    /// No file at the resolved path.
    Absent(String),
    /// The file exists but is not the verified artifact.
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    /// An export the ABI needs did not resolve.
    MissingExport(String),
    /// The plan asks for something the ABI cannot represent.
    InvalidInput(String),
    /// No call has a usable hardware backend under the pinned policy.
    NoBackend(String),
    /// The collecting export reported `ERROR_RESULT`.
    Rejected(String),
    /// The forward matcher reported an unexpected native status.
    Status(i32),
}

impl std::fmt::Display for PreimageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreimageError::Absent(message)
            | PreimageError::MissingExport(message)
            | PreimageError::InvalidInput(message)
            | PreimageError::NoBackend(message)
            | PreimageError::Rejected(message) => formatter.write_str(message),
            PreimageError::HashMismatch {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "the effect-preimage accelerator at {path} is not the verified artifact: \
                 expected sha256 {expected}, found {actual}"
            ),
            PreimageError::Status(status) => write!(
                formatter,
                "the Direct3D 11 effect matcher failed with native status {status}"
            ),
        }
    }
}

/// The scalar parameters of one compiled plan's native sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreimagePlanParams {
    pub pivot_draw_index: u32,
    pub pivot_affine_addend: u32,
    pub pivot_inverse_multiplier: u32,
    pub promotion_draw_index: u32,
    pub promotion_probability_percent: u32,
    pub shuffle_draw_start: u32,
    pub rarity: u8,
    pub slot_limit: u8,
    pub maximum_draw: u32,
}

/// One forward-filter request (`match_effect_constraints_d3d11`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectMaskRequest {
    pub seeds: Vec<u32>,
    pub candidates: Vec<EffectCandidateInput>,
    pub special_groups: Vec<SpecialGroupInput>,
    pub category_capacities: Vec<u32>,
    /// `(kind, keys)` pairs, in the shipped order.
    pub criterion_groups: Vec<(u32, Vec<u32>)>,
    pub rarity: u32,
    pub ordinary_slot_count: u32,
    pub slot_limit: u32,
    pub promotion_threshold: u32,
    pub consumes_special_draw: bool,
    pub minimum_roll_percent: u32,
    pub maximum_roll_percent: u32,
    pub apply_r4_finalizer: bool,
    pub auxiliary_mode_threshold: u32,
    pub preferred_vendor_id: u32,
}

/// Hash one candidate library file.
pub fn file_sha256(path: &Path) -> Result<String, PreimageError> {
    let bytes = std::fs::read(path).map_err(|error| {
        PreimageError::Absent(format!(
            "the effect-preimage accelerator at {} is unreadable: {error}",
            path.display()
        ))
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Resolve the module path the product loads.
pub fn resolve_module_path(application_root: &Path, override_path: Option<&Path>) -> PathBuf {
    match override_path {
        Some(path) => path.to_path_buf(),
        None => application_root.join("bin").join(PREIMAGE_LIBRARY),
    }
}

/// A loaded, hash-pinned effect-preimage accelerator.
pub struct PreimageAccelerator {
    native: platform::Library,
    identity: PreimageIdentity,
    last_vendor_id: Mutex<Option<u32>>,
}

impl PreimageAccelerator {
    /// Load the verified library, or explain exactly why it is unusable.
    ///
    /// The hash is checked before the module is mapped: a substituted artifact
    /// must never execute, and an unknown override is reported rather than
    /// silently accepted.
    pub fn load(
        application_root: &Path,
        override_path: Option<&Path>,
    ) -> Result<Self, PreimageError> {
        let path = resolve_module_path(application_root, override_path);
        if !path.is_file() {
            return Err(PreimageError::Absent(format!(
                "the shipped effect-preimage accelerator is missing at {}",
                path.display()
            )));
        }
        let actual = file_sha256(&path)?;
        if actual != PREIMAGE_SHA256 {
            return Err(PreimageError::HashMismatch {
                path: path.display().to_string(),
                expected: PREIMAGE_SHA256.to_string(),
                actual,
            });
        }
        let native = platform::Library::load(&path)?;
        let identity = PreimageIdentity {
            path,
            sha256: actual,
            exports: platform::EXPORTS.to_vec(),
        };
        Ok(Self {
            native,
            identity,
            last_vendor_id: Mutex::new(None),
        })
    }

    /// The pinned identity of the loaded artifact.
    pub fn identity(&self) -> &PreimageIdentity {
        &self.identity
    }

    /// A real probe of `d3d11_effect_acceleration_available`.
    pub fn available(&self) -> bool {
        self.native.available(self.configured_vendor_id())
    }

    /// A real probe of `d3d11_effect_adapter_info`.
    pub fn adapter_info(&self) -> Option<PreimageAdapterInfo> {
        self.native.adapter_info(self.configured_vendor_id())
    }

    /// The vendor `NIOH3_D3D11_VENDOR` selects, or `0` for "driver's choice".
    pub fn configured_vendor_id(&self) -> u32 {
        platform::configured_vendor_id()
    }

    /// Fail closed under the pinned policy when no hardware backend is usable.
    ///
    /// The two policies keep distinct, named reasons: neither of them invents a
    /// replay, and the strict policy never quietly degrades into one.
    pub fn require_backend(&self, policy: PreimagePolicy) -> Result<(), PreimageError> {
        if self.available() {
            return Ok(());
        }
        Err(match policy {
            PreimagePolicy::StrictGpu => PreimageError::NoBackend(
                "the effect-preimage route needs a DirectCompute GPU backend and none is \
                 usable on this machine; the strict-GPU policy refuses a slow replay"
                    .to_string(),
            ),
            PreimagePolicy::AllowCpuFallback => PreimageError::NoBackend(
                "no DirectCompute backend is usable and this worker has no certified \
                 non-accelerated replay for the effect-preimage route yet"
                    .to_string(),
            ),
        })
    }

    /// Sweep one bounded plan window for `(Seed, zero-based plan trial)` pairs.
    #[allow(clippy::too_many_arguments)]
    pub fn collect_matches(
        &self,
        pivot_values: &[u16],
        paths: &[EffectPathInput],
        params: &PreimagePlanParams,
        start_trial: u64,
        stop_trial: u64,
        output_capacity: usize,
        preferred_vendor_id: u32,
    ) -> Result<Vec<(u32, u64)>, PreimageError> {
        if pivot_values.is_empty() {
            return Err(PreimageError::InvalidInput(
                "the compiled pivot preimage cannot be empty".to_string(),
            ));
        }
        if paths.is_empty() {
            return Err(PreimageError::InvalidInput(
                "the compiled plan contains no native paths".to_string(),
            ));
        }
        let family_size = pivot_values.len() as u64 * 0x1_0000;
        if start_trial > stop_trial || stop_trial > family_size {
            return Err(PreimageError::InvalidInput(
                "invalid compiled effect pivot range".to_string(),
            ));
        }
        if !(MIN_OUTPUT_CAPACITY..=MAX_OUTPUT_CAPACITY).contains(&output_capacity) {
            return Err(PreimageError::InvalidInput(
                "output_capacity must be in 1..=1_000_000".to_string(),
            ));
        }
        if start_trial == stop_trial {
            return Ok(Vec::new());
        }
        let _guard = PREIMAGE_CALL_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let (count, seeds, trials, vendor_id) = self.native.collect_matches(
            pivot_values,
            paths,
            params,
            start_trial,
            stop_trial,
            output_capacity,
            preferred_vendor_id,
        );
        if count == ERROR_RESULT {
            return Err(PreimageError::Rejected(
                "the Direct3D 11 effect-preimage accelerator rejected the plan".to_string(),
            ));
        }
        *self
            .last_vendor_id
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(vendor_id);
        let mut matches: Vec<(u32, u64)> =
            seeds.into_iter().zip(trials).take(count as usize).collect();
        matches.sort_by_key(|item| item.1);
        Ok(matches)
    }

    /// Decide a Seed batch with the DirectCompute forward filter.
    ///
    /// `Ok(None)` is the library's own "no backend" answer (`-2`), which the
    /// caller must treat as an unserved route rather than as a rejection.
    pub fn match_effect_constraints(
        &self,
        request: &EffectMaskRequest,
    ) -> Result<Option<Vec<u32>>, PreimageError> {
        if request.seeds.is_empty() || request.seeds.len() > MAX_PREDICATE_BATCH {
            return Err(PreimageError::InvalidInput(
                "DirectCompute effect batches require 1..=1,000,000 Seeds".to_string(),
            ));
        }
        if request.candidates.is_empty() || request.candidates.len() > MAX_CANDIDATES {
            return Err(PreimageError::InvalidInput(
                "the DirectCompute effect candidate table is invalid".to_string(),
            ));
        }
        if !matches!(request.special_groups.len(), 1 | 0x1_0000) {
            return Err(PreimageError::InvalidInput(
                "the special group lookup must contain 1 or 65,536 entries".to_string(),
            ));
        }
        if request.category_capacities.len() != CATEGORY_CAPACITY_SLOTS {
            return Err(PreimageError::InvalidInput(
                "category capacities must contain 32 entries".to_string(),
            ));
        }
        if request.criterion_groups.is_empty()
            || request.criterion_groups.len() > MAX_CRITERION_GROUPS
        {
            return Err(PreimageError::InvalidInput(
                "effect matching requires 1..=32 criterion groups".to_string(),
            ));
        }
        let _guard = PREIMAGE_CALL_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let (status, mask, vendor_id) = self.native.match_effect_constraints(request);
        if status == STATUS_UNAVAILABLE {
            return Ok(None);
        }
        if status != 1 {
            return Err(PreimageError::Status(status));
        }
        *self
            .last_vendor_id
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(vendor_id);
        Ok(Some(mask))
    }

    /// `last_effect_preimage_backend()`.
    pub fn last_backend(&self) -> PreimageBackend {
        match *self
            .last_vendor_id
            .lock()
            .unwrap_or_else(|error| error.into_inner())
        {
            Some(vendor_id) => PreimageBackend::from_vendor(vendor_id),
            None => PreimageBackend::NotUsed,
        }
    }

    /// `reset_effect_preimage_backend()`.
    pub fn reset_backend(&self) {
        *self
            .last_vendor_id
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
    }
}

impl std::fmt::Debug for PreimageAccelerator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreimageAccelerator")
            .field("identity", &self.identity)
            .field("last_backend", &self.last_backend())
            .finish()
    }
}

#[cfg(windows)]
mod platform {
    use std::ffi::{c_void, OsStr};
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use super::{
        EffectCandidateInput, EffectMaskRequest, EffectPathInput, PreimageAdapterInfo,
        PreimageError, PreimagePlanParams, SpecialGroupInput, ERROR_RESULT,
    };

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
        fn GetEnvironmentVariableW(name: *const u16, buffer: *mut u16, size: u32) -> u32;
    }

    /// The exports this module resolves, in `ctypes` lookup order.
    pub(super) const EXPORTS: [&str; 4] = [
        "d3d11_effect_acceleration_available",
        "d3d11_effect_adapter_info",
        "collect_effect_preimage_matches_d3d11",
        "match_effect_constraints_d3d11",
    ];

    type Availability = unsafe extern "C" fn(u32) -> i32;
    #[allow(clippy::type_complexity)]
    type AdapterInfo =
        unsafe extern "C" fn(u32, *mut u32, *mut u32, *mut u64, *mut u64, *mut u16, u32) -> i32;
    #[allow(clippy::type_complexity)]
    type CollectMatches = unsafe extern "C" fn(
        *const u16,
        u32,
        u64,
        u64,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        *const EffectPathInput,
        u32,
        u32,
        *mut u32,
        *mut u64,
        u32,
        *mut u32,
    ) -> u64;
    #[allow(clippy::type_complexity)]
    type MatchEffects = unsafe extern "C" fn(
        *const u32,
        u32,
        *const EffectCandidateInput,
        u32,
        *const SpecialGroupInput,
        u32,
        *const u32,
        *const u32,
        u32,
        *const u32,
        *const u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        *mut u32,
        *mut u32,
    ) -> i32;

    /// One loaded module with its required symbols resolved.
    pub(super) struct Library {
        availability: Availability,
        adapter_info: AdapterInfo,
        collect: CollectMatches,
        match_effects: MatchEffects,
    }

    // SAFETY: the library is never unloaded and every call is serialized by the
    // module-level call lock, which is the same guarantee the seed layer relies
    // on for its accelerator handle.
    unsafe impl Send for Library {}
    unsafe impl Sync for Library {}

    impl Library {
        pub(super) fn load(module_path: &Path) -> Result<Self, PreimageError> {
            let mut wide: Vec<u16> = module_path.as_os_str().encode_wide().collect();
            wide.push(0);
            // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the
            // call; the handle is intentionally never freed.
            let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
            if handle.is_null() {
                return Err(PreimageError::Absent(format!(
                    "the verified effect-preimage accelerator at {} could not be mapped",
                    module_path.display()
                )));
            }
            // SAFETY: each symbol name is a static NUL-terminated byte string.
            let availability: Availability = unsafe {
                symbol(handle, b"d3d11_effect_acceleration_available\0").ok_or_else(missing)?
            };
            let adapter_info: AdapterInfo =
                unsafe { symbol(handle, b"d3d11_effect_adapter_info\0").ok_or_else(missing)? };
            let collect: CollectMatches = unsafe {
                symbol(handle, b"collect_effect_preimage_matches_d3d11\0").ok_or_else(missing)?
            };
            let match_effects: MatchEffects =
                unsafe { symbol(handle, b"match_effect_constraints_d3d11\0").ok_or_else(missing)? };
            Ok(Self {
                availability,
                adapter_info,
                collect,
                match_effects,
            })
        }

        pub(super) fn available(&self, preferred_vendor_id: u32) -> bool {
            // SAFETY: the export takes one integer and returns one integer.
            unsafe { (self.availability)(preferred_vendor_id) != 0 }
        }

        pub(super) fn adapter_info(&self, preferred_vendor_id: u32) -> Option<PreimageAdapterInfo> {
            let mut actual_vendor: u32 = 0;
            let mut device_id: u32 = 0;
            let mut dedicated: u64 = 0;
            let mut shared: u64 = 0;
            let mut description = [0u16; 128];
            // SAFETY: every pointer refers to a live, correctly typed local for
            // the duration of the call, and the capacity matches the buffer.
            let ok = unsafe {
                (self.adapter_info)(
                    preferred_vendor_id,
                    &mut actual_vendor,
                    &mut device_id,
                    &mut dedicated,
                    &mut shared,
                    description.as_mut_ptr(),
                    description.len() as u32,
                )
            };
            if ok == 0 {
                return None;
            }
            // The export writes a NUL-terminated UTF-16 description; the buffer
            // is already the caller's live local, so it is read directly rather
            // than through a narrow `CStr` that would stop at the first zero byte.
            let end = description
                .iter()
                .position(|value| *value == 0)
                .unwrap_or(description.len());
            Some(PreimageAdapterInfo {
                description: String::from_utf16_lossy(&description[..end]),
                vendor_id: actual_vendor,
                device_id,
                dedicated_video_memory: dedicated,
                shared_system_memory: shared,
            })
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn collect_matches(
            &self,
            pivot_values: &[u16],
            paths: &[EffectPathInput],
            params: &PreimagePlanParams,
            start_trial: u64,
            stop_trial: u64,
            output_capacity: usize,
            preferred_vendor_id: u32,
        ) -> (u64, Vec<u32>, Vec<u64>, u32) {
            let mut seeds = vec![0u32; output_capacity];
            let mut trials = vec![0u64; output_capacity];
            let mut actual_vendor: u32 = 0;
            // SAFETY: every pointer refers to a live allocation of the length
            // the ABI is told about, and the arrays outlive the call.
            let count = unsafe {
                (self.collect)(
                    pivot_values.as_ptr(),
                    pivot_values.len() as u32,
                    start_trial,
                    stop_trial,
                    params.pivot_draw_index,
                    params.pivot_affine_addend,
                    params.pivot_inverse_multiplier,
                    params.promotion_draw_index,
                    params.promotion_probability_percent * 100,
                    params.shuffle_draw_start,
                    u32::from(params.rarity),
                    u32::from(params.slot_limit),
                    params.maximum_draw,
                    paths.as_ptr(),
                    paths.len() as u32,
                    preferred_vendor_id,
                    seeds.as_mut_ptr(),
                    trials.as_mut_ptr(),
                    output_capacity as u32,
                    &mut actual_vendor,
                )
            };
            if count == ERROR_RESULT {
                return (count, Vec::new(), Vec::new(), actual_vendor);
            }
            (count, seeds, trials, actual_vendor)
        }

        pub(super) fn match_effect_constraints(
            &self,
            request: &EffectMaskRequest,
        ) -> (i32, Vec<u32>, u32) {
            let mut flattened: Vec<u32> = Vec::new();
            let mut offsets: Vec<u32> = vec![0];
            let mut kinds: Vec<u32> = Vec::new();
            for (kind, group) in &request.criterion_groups {
                flattened.extend(group.iter().copied());
                offsets.push(flattened.len() as u32);
                kinds.push(*kind);
            }
            let mut output = vec![0u32; request.seeds.len()];
            let mut actual_vendor: u32 = 0;
            // SAFETY: every pointer refers to a live allocation of the length
            // the ABI is told about, and the arrays outlive the call.
            let status = unsafe {
                (self.match_effects)(
                    request.seeds.as_ptr(),
                    request.seeds.len() as u32,
                    request.candidates.as_ptr(),
                    request.candidates.len() as u32,
                    request.special_groups.as_ptr(),
                    request.special_groups.len() as u32,
                    request.category_capacities.as_ptr(),
                    flattened.as_ptr(),
                    flattened.len() as u32,
                    offsets.as_ptr(),
                    kinds.as_ptr(),
                    kinds.len() as u32,
                    request.rarity,
                    request.ordinary_slot_count,
                    request.slot_limit,
                    request.promotion_threshold,
                    u32::from(request.consumes_special_draw),
                    request.minimum_roll_percent,
                    request.maximum_roll_percent,
                    u32::from(request.apply_r4_finalizer),
                    request.auxiliary_mode_threshold,
                    request.preferred_vendor_id,
                    output.as_mut_ptr(),
                    &mut actual_vendor,
                )
            };
            (status, output, actual_vendor)
        }
    }

    fn missing() -> PreimageError {
        PreimageError::MissingExport(
            "the verified effect-preimage accelerator does not export the ABI this worker \
             requires"
                .to_string(),
        )
    }

    /// Resolve one exported symbol into a typed function pointer.
    unsafe fn symbol<T: Copy>(handle: *mut c_void, name: &[u8]) -> Option<T> {
        let address = unsafe { GetProcAddress(handle, name.as_ptr()) };
        if address.is_null() {
            return None;
        }
        Some(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&address) })
    }

    /// `NIOH3_D3D11_VENDOR`, mirroring the Python module's parser.
    pub(super) fn configured_vendor_id() -> u32 {
        let name: Vec<u16> = OsStr::new("NIOH3_D3D11_VENDOR")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut buffer = [0u16; 32];
        // SAFETY: `name` is NUL-terminated and `buffer` is a live local.
        let length = unsafe { GetEnvironmentVariableW(name.as_ptr(), buffer.as_mut_ptr(), 32) };
        if length == 0 || length >= 32 {
            return 0;
        }
        let raw = String::from_utf16_lossy(&buffer[..length as usize]);
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return 0;
        }
        if let Some(hex) = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
        {
            return u32::from_str_radix(hex, 16).unwrap_or(0);
        }
        trimmed.parse::<u32>().unwrap_or(0)
    }
}

#[cfg(not(windows))]
mod platform {
    use std::path::Path;

    use super::{
        EffectMaskRequest, EffectPathInput, PreimageAdapterInfo, PreimageError, PreimagePlanParams,
        ERROR_RESULT,
    };

    pub(super) const EXPORTS: [&str; 4] = [
        "d3d11_effect_acceleration_available",
        "d3d11_effect_adapter_info",
        "collect_effect_preimage_matches_d3d11",
        "match_effect_constraints_d3d11",
    ];

    pub(super) struct Library;

    impl Library {
        pub(super) fn load(_module_path: &Path) -> Result<Self, PreimageError> {
            Err(PreimageError::Absent(
                "the Direct3D 11 effect-preimage accelerator is Windows-only".to_string(),
            ))
        }

        pub(super) fn available(&self, _preferred_vendor_id: u32) -> bool {
            false
        }

        pub(super) fn adapter_info(
            &self,
            _preferred_vendor_id: u32,
        ) -> Option<PreimageAdapterInfo> {
            None
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn collect_matches(
            &self,
            _pivot_values: &[u16],
            _paths: &[EffectPathInput],
            _params: &PreimagePlanParams,
            _start_trial: u64,
            _stop_trial: u64,
            _output_capacity: usize,
            _preferred_vendor_id: u32,
        ) -> (u64, Vec<u32>, Vec<u64>, u32) {
            (ERROR_RESULT, Vec::new(), Vec::new(), 0)
        }

        pub(super) fn match_effect_constraints(
            &self,
            _request: &EffectMaskRequest,
        ) -> (i32, Vec<u32>, u32) {
            (super::STATUS_UNAVAILABLE, Vec::new(), 0)
        }
    }

    pub(super) fn configured_vendor_id() -> u32 {
        0
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::effect_path::{
        compile_full_composition_plans, native_path_descriptors, FullCompositionRequest,
    };

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// The tracked offline vectors these tests read, beside the crate because
    /// `deliverables/` is git-ignored and therefore absent from a clean checkout.
    fn fixtures_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("m23d-preimage")
            .join("evidence")
    }

    fn data_root() -> PathBuf {
        repo_root().join("nioh3_scroll_editor").join("data")
    }

    fn shipped_tables() -> nioh3_domain::effect::EffectTableIndex {
        let bytes = nioh3_data::load_effect_resource(&data_root()).expect("effect resource");
        nioh3_domain::effect::EffectTableIndex::from_resource(&bytes).expect("effect tables")
    }

    fn vector() -> serde_json::Value {
        let path = fixtures_root().join("complete_r3_plans.json");
        serde_json::from_str(&std::fs::read_to_string(&path).expect("vector")).expect("json")
    }

    /// A missing library is a named failure, never a panic and never a silent
    /// substitution.
    #[test]
    fn a_missing_library_reports_a_named_absence() {
        let missing = repo_root()
            .join("bin")
            .join("not-the-preimage-accelerator.dll");
        let error = PreimageAccelerator::load(&repo_root(), Some(&missing))
            .expect_err("a missing library cannot load");
        assert!(matches!(error, PreimageError::Absent(_)), "{error:?}");
        assert!(error.to_string().contains("missing"));
    }

    /// An override that is not the verified artifact is rejected by hash before
    /// it can execute, and the report names both hashes.
    #[test]
    fn an_unknown_override_is_rejected_by_hash() {
        let other = repo_root().join("bin").join("nioh3_seed_accelerator.dll");
        let error = PreimageAccelerator::load(&repo_root(), Some(&other))
            .expect_err("a different artifact cannot load");
        match &error {
            PreimageError::HashMismatch {
                expected, actual, ..
            } => {
                assert_eq!(expected, PREIMAGE_SHA256);
                assert_ne!(actual, PREIMAGE_SHA256);
                assert_eq!(*actual, file_sha256(&other).expect("hash"));
            }
            other => panic!("expected a hash mismatch, got {other:?}"),
        }
    }

    /// The shipped artifact loads, matches the pinned hash and exposes the four
    /// documented exports.
    #[test]
    fn the_shipped_library_loads_with_the_pinned_identity() {
        let accelerator =
            PreimageAccelerator::load(&repo_root(), None).expect("the shipped library loads");
        let identity = accelerator.identity();
        assert_eq!(identity.sha256, PREIMAGE_SHA256);
        assert_eq!(identity.exports, platform::EXPORTS.to_vec());
        assert!(identity.path.ends_with(PREIMAGE_LIBRARY));
        assert_eq!(accelerator.last_backend(), PreimageBackend::NotUsed);
    }

    /// The capability probe is real: availability and adapter info agree, and an
    /// unusable backend can never be reported as usable.
    #[test]
    fn the_capability_probe_is_real() {
        let accelerator =
            PreimageAccelerator::load(&repo_root(), None).expect("the shipped library loads");
        let available = accelerator.available();
        let info = accelerator.adapter_info();
        assert_eq!(
            available,
            info.is_some(),
            "probe and adapter info must agree"
        );
        if let Some(info) = info {
            assert!(!info.description.is_empty());
            assert_ne!(info.vendor_id, 0);
        }
        assert_eq!(
            accelerator
                .require_backend(PreimagePolicy::StrictGpu)
                .is_ok(),
            available
        );
    }

    /// The whole chain end to end: ported plan -> packed descriptors -> the real
    /// DirectCompute sweep must reproduce the shipped reference window exactly.
    #[test]
    fn the_real_sweep_reproduces_the_reference_window() {
        let accelerator =
            PreimageAccelerator::load(&repo_root(), None).expect("the shipped library loads");
        if !accelerator.available() {
            // A machine without a DirectCompute device cannot serve this route;
            // the refusal must be named rather than silently empty.
            let error = accelerator
                .require_backend(PreimagePolicy::StrictGpu)
                .expect_err("no device means no route");
            assert!(matches!(error, PreimageError::NoBackend(_)), "{error:?}");
            return;
        }
        let window: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(fixtures_root().join("preimage_windows.json"))
                .expect("window vector"),
        )
        .expect("json");
        let plan_vector = vector();
        let request = FullCompositionRequest {
            rarity: 3,
            primary_effect_id: plan_vector["primary_effect_ids"][0].as_u64().unwrap() as u32,
            secondary_effect_ids: plan_vector["required_secondary_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as u32)
                .collect(),
            stage_special_effect_id: None,
            natural_only: true,
            playthrough: 3,
        };
        let plans =
            compile_full_composition_plans(&request, &shipped_tables(), &[]).expect("plans");
        let plan = &plans[0];
        let descriptors = native_path_descriptors(plan).expect("descriptors");
        let maximum_draw = plan
            .paths
            .iter()
            .flat_map(|path| path.constraints.iter().map(|item| item.draw_index))
            .max()
            .expect("plans carry constraints");
        let params = PreimagePlanParams {
            pivot_draw_index: plan.pivot_draw_index,
            pivot_affine_addend: plan.pivot_affine_addend,
            pivot_inverse_multiplier: plan.pivot_inverse_multiplier,
            promotion_draw_index: plan.promotion_draw_index,
            promotion_probability_percent: plan.promotion_probability_percent,
            shuffle_draw_start: plan.shuffle_draw_start,
            rarity: plan.request.rarity(),
            slot_limit: plan.slot_limit,
            maximum_draw,
        };
        let window_trials = window["window_trials"].as_u64().unwrap();
        let pivot_values: Vec<u16> = plan
            .pivot_allowed_u16
            .iter()
            .flat_map(|run| run.start..=run.end)
            .collect();
        let matches = accelerator
            .collect_matches(
                &pivot_values,
                &descriptors,
                &params,
                0,
                window_trials.min(plan.pivot_state_count()),
                DEFAULT_OUTPUT_CAPACITY,
                accelerator.configured_vendor_id(),
            )
            .expect("the real sweep runs");
        let expected: Vec<(u32, u64)> = window["per_plan"][0]["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| (pair[0].as_u64().unwrap() as u32, pair[1].as_u64().unwrap()))
            .collect();
        assert!(
            !expected.is_empty(),
            "the reference window must contain hits"
        );
        assert_eq!(matches, expected);
        match accelerator.last_backend() {
            PreimageBackend::NotUsed => panic!("a completed sweep must report its backend"),
            backend => {
                assert!(backend.as_str().starts_with("d3d11_"), "{backend:?}");
            }
        }
        accelerator.reset_backend();
        assert_eq!(accelerator.last_backend(), PreimageBackend::NotUsed);
    }

    /// The forward filter binding must reproduce the shipped mask batch.
    #[test]
    fn the_forward_filter_reproduces_the_reference_masks() {
        let accelerator =
            PreimageAccelerator::load(&repo_root(), None).expect("the shipped library loads");
        if !accelerator.available() {
            return;
        }
        let vector: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(fixtures_root().join("forward_filter_masks.json"))
                .expect("mask vector"),
        )
        .expect("json");
        let typed = |key: &str, fields: usize| -> Vec<Vec<u32>> {
            vector[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    let values: Vec<u32> = row
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| value.as_u64().unwrap() as u32)
                        .collect();
                    assert_eq!(values.len(), fields, "{key} row width");
                    values
                })
                .collect()
        };
        let mut candidates = Vec::new();
        for row in typed("candidate_rows", 11) {
            candidates.push(EffectCandidateInput {
                effect_id: row[0],
                group_key: row[1],
                category_key: row[2],
                conflict_mask_0: row[3],
                conflict_mask_1: row[4],
                normal_weight: row[5],
                promoted_weight: row[6],
                final_weight_common: row[7],
                final_weight_special: row[8],
                completion_candidate: row[9],
                value_one_roll_mask: row[10],
            });
        }
        let mut special_groups = Vec::new();
        for row in typed("special_groups", 4) {
            special_groups.push(SpecialGroupInput {
                group_key: row[0],
                conflict_mask_0: row[1],
                conflict_mask_1: row[2],
                effect_id: row[3],
            });
        }
        let criterion_groups: Vec<(u32, Vec<u32>)> = vector["criterion_groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|group| {
                (
                    group[0].as_u64().unwrap() as u32,
                    group[1]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| value.as_u64().unwrap() as u32)
                        .collect(),
                )
            })
            .collect();
        let request = EffectMaskRequest {
            seeds: vector["seeds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as u32)
                .collect(),
            candidates,
            special_groups,
            category_capacities: vector["category_capacities"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as u32)
                .collect(),
            criterion_groups,
            rarity: vector["rarity"].as_u64().unwrap() as u32,
            ordinary_slot_count: vector["ordinary_slot_count"].as_u64().unwrap() as u32,
            slot_limit: vector["slot_limit"].as_u64().unwrap() as u32,
            promotion_threshold: vector["promotion_threshold"].as_u64().unwrap() as u32,
            consumes_special_draw: vector["consumes_special_draw"].as_bool().unwrap(),
            minimum_roll_percent: vector["minimum_roll_percent"].as_u64().unwrap() as u32,
            maximum_roll_percent: vector["maximum_roll_percent"].as_u64().unwrap() as u32,
            apply_r4_finalizer: vector["apply_r4_finalizer"].as_bool().unwrap(),
            auxiliary_mode_threshold: 2000,
            preferred_vendor_id: accelerator.configured_vendor_id(),
        };
        let mask = accelerator
            .match_effect_constraints(&request)
            .expect("the forward filter runs")
            .expect("a backend is available");
        let expected: Vec<u32> = vector["masks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect();
        assert_eq!(mask, expected);
        assert_eq!(
            vector["target_mask"].as_u64().unwrap() as u32,
            (1 << request.criterion_groups.len()) - 1
        );
    }
}
