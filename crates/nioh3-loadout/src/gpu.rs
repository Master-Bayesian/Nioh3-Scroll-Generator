//! The graphics-card backend of the search. An attack bundle may carry `kernel.dll`, generated from the same
//! compiled paths and built with CUDA; it runs this crate's pipeline and formula on the card. The library is only
//! code that claims to agree with the engine: a search trusts it after it gives the engine's exact outcome on a
//! sample of that search's own outfits (see `search.rs`), and any load failure, error or disagreement sends the
//! search back to the CPU. Machines without an NVIDIA driver never get past loading.

use crate::context::EncodedContext;
use crate::engine::{Candidate, Limits, ITEM, SLOTS};
use std::borrow::Cow;
use std::ffi::{c_char, c_void};
use std::path::Path;
use std::time::Instant;

#[path = "gpu_prepared_reuse.rs"]
mod prepared_reuse;
#[path = "gpu_prepared_types.rs"]
mod prepared_types;
pub use prepared_reuse::PreparedReuseStats;
pub(crate) use prepared_types::PreparedDamage;

/// The library's interface version; a library that reports another one is not used.
const ABI: u32 = 1;
const ERROR_LEN: usize = 1024;
const PRODUCT_CONTRACT_VERSION: u32 = 20261115;

#[repr(C)]
#[derive(Clone, Copy)]
struct NkFile {
    name: *const c_char,
    data: *const u8,
    size: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NkLimits {
    on: u32,
    max_load: f32,
    min_defense: u32,
    min_health: u32,
}

// Optional native-set ABI 1 retains the full public usize thresholds as u64
// on the supported 64-bit host. Duplicate options remain separate entries.
#[repr(C)]
#[derive(Clone, Copy)]
struct NkSetOption {
    key: u32,
    reserved: u32,
    pieces: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct NkSetRule {
    first: u64,
    n: u64,
    at_least: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct NkSets {
    rules: *const NkSetRule,
    rules_n: u64,
    options: *const NkSetOption,
    options_n: u64,
}
const _: () = assert!(std::mem::size_of::<NkSetOption>() == 16);
const _: () = assert!(std::mem::size_of::<NkSetRule>() == 24);
const _: () = assert!(std::mem::size_of::<NkSets>() == 32);

/// Own both flattened arrays until the synchronous foreign call returns.
struct SetInput {
    rules: Vec<NkSetRule>,
    options: Vec<NkSetOption>,
}
impl SetInput {
    fn new(sets: &[crate::search::SetRule]) -> Result<Self, String> {
        let mut rules = Vec::with_capacity(sets.len());
        let mut options = Vec::new();
        for rule in sets {
            rules.push(NkSetRule {
                first: u64::try_from(options.len()).map_err(|_| "too many set options")?,
                n: u64::try_from(rule.options.len()).map_err(|_| "too many set options")?,
                at_least: u64::try_from(rule.at_least).map_err(|_| "set threshold exceeds u64")?,
            });
            for option in &rule.options {
                options.push(NkSetOption {
                    key: option.key,
                    reserved: 0,
                    pieces: u64::try_from(option.pieces).map_err(|_| "set pieces exceed u64")?,
                });
            }
        }
        Ok(Self { rules, options })
    }

    fn descriptor(&self) -> Result<NkSets, String> {
        Ok(NkSets {
            rules: self.rules.as_ptr(),
            rules_n: u64::try_from(self.rules.len()).map_err(|_| "too many set rules")?,
            options: self.options.as_ptr(),
            options_n: u64::try_from(self.options.len()).map_err(|_| "too many set options")?,
        })
    }
}

/// One space as index lists: position a of outfit `index` takes `lists[offset[a] + digit_a]`, digits least
/// significant first, exactly as `search::Space::candidate` numbers its outfits.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NkSpace {
    pub n: u32,
    pub slot: [i32; SLOTS],
    pub offset: [u32; SLOTS],
    pub radix: [u32; SLOTS],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct NkHit {
    pub bits: u32,
    pub space: u32,
    pub index: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NkCand {
    attrs: [i32; 7],
    slot: [i32; SLOTS],
}

/// What one card evaluation of a case said, in the engine's terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardOutcome {
    Refused,
    Damage(u32),
    NoPath,
    OutsideLimits,
}

/// Totals and kept answers of one card sweep.
pub struct Sweep {
    /// Refused, answered, no path, outside limits.
    pub counts: [u64; 4],
    /// Every hit the card kept, in no order.
    pub hits: Vec<NkHit>,
    /// How many hits there were; more than `hits.len()` when the buffer was too small.
    pub hit_total: u64,
    /// Concatenated outfit indexes without a path, at most the capacity asked for.
    pub missed: Vec<u64>,
}

type AbiFn = unsafe extern "C" fn() -> u32;
type OpenFn = unsafe extern "C" fn(*const NkFile, u32, *mut c_char, u32) -> *mut c_void;
type PoolFn = unsafe extern "C" fn(*mut c_void, *const u8, u64, *mut c_char, u32) -> i32;
type CasesFn = unsafe extern "C" fn(
    *mut c_void,
    *const NkCand,
    u64,
    *const NkLimits,
    *mut u32,
    *mut u8,
    *mut c_char,
    u32,
) -> i32;
type SearchFn = unsafe extern "C" fn(
    *mut c_void,
    *const NkSpace,
    u32,
    *const u32,
    u64,
    *const i32,
    *const NkLimits,
    f32,
    u32,
    *mut NkHit,
    u32,
    *mut u32,
    *mut u64,
    *mut u64,
    u32,
    *mut u32,
    *const u32,
    *mut c_char,
    u32,
) -> i32;
type SearchObserverFn = unsafe extern "C" fn(*mut c_void, u64, u64) -> u32;
type ObservedSearchFn = unsafe extern "C" fn(
    *mut c_void,
    *const NkSpace,
    u32,
    *const u32,
    u64,
    *const i32,
    *const NkLimits,
    f32,
    u32,
    *mut NkHit,
    u32,
    *mut u32,
    *mut u64,
    *mut u64,
    u32,
    *mut u32,
    Option<SearchObserverFn>,
    *mut c_void,
    *mut c_char,
    u32,
) -> i32;

type CasesSetsFn = unsafe extern "C" fn(
    *mut c_void,
    *const NkCand,
    u64,
    *const NkLimits,
    *const NkSets,
    *mut u32,
    *mut u8,
    *mut c_char,
    u32,
) -> i32;
type SearchSetsFn = unsafe extern "C" fn(
    *mut c_void,
    *const NkSpace,
    u32,
    *const u32,
    u64,
    *const i32,
    *const NkLimits,
    *const NkSets,
    f32,
    u32,
    *mut NkHit,
    u32,
    *mut u32,
    *mut u64,
    *mut u64,
    u32,
    *mut u32,
    *const u32,
    *mut c_char,
    u32,
) -> i32;
type ObservedSearchSetsFn = unsafe extern "C" fn(
    *mut c_void,
    *const NkSpace,
    u32,
    *const u32,
    u64,
    *const i32,
    *const NkLimits,
    *const NkSets,
    f32,
    u32,
    *mut NkHit,
    u32,
    *mut u32,
    *mut u64,
    *mut u64,
    u32,
    *mut u32,
    Option<SearchObserverFn>,
    *mut c_void,
    *mut c_char,
    u32,
) -> i32;

#[derive(Clone, Copy)]
struct SetPredicate {
    cases: CasesSetsFn,
    search: SearchSetsFn,
    observed_search: ObservedSearchSetsFn,
}
impl SetPredicate {
    fn load(lib: &os::Library) -> Option<Self> {
        let query = lib.symbol(b"nk_set_predicate_abi\0")?;
        // SAFETY: the optional capability query has no arguments.
        let version = unsafe { std::mem::transmute::<*mut c_void, AbiFn>(query)() };
        if version != 1 {
            return None;
        }
        // Require the complete capability. A partially replaced DLL cannot
        // validate a sweep while silently omitting the predicate in polishing.
        let cases = lib.symbol(b"nk_cases_sets\0")?;
        let search = lib.symbol(b"nk_search_sets\0")?;
        let observed_search = lib.symbol(b"nk_search_observed_sets\0")?;
        // SAFETY: capability ABI 1 defines these exact synchronous signatures.
        Some(unsafe {
            Self {
                cases: std::mem::transmute::<*mut c_void, CasesSetsFn>(cases),
                search: std::mem::transmute::<*mut c_void, SearchSetsFn>(search),
                observed_search: std::mem::transmute::<*mut c_void, ObservedSearchSetsFn>(
                    observed_search,
                ),
            }
        })
    }
}

#[derive(Debug)]
pub enum GpuSweepError {
    Failure(String),
    Cancelled,
    ObserverFailed,
}
impl std::fmt::Display for GpuSweepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failure(message) => f.write_str(message),
            Self::Cancelled => f.write_str("search cancelled"),
            Self::ObserverFailed => f.write_str("search observer failed"),
        }
    }
}
impl std::error::Error for GpuSweepError {}
struct ObserverCall<'a> {
    callback: &'a mut dyn FnMut(u64, u64) -> bool,
    panicked: bool,
}
unsafe extern "C" fn observe_search(user: *mut c_void, completed: u64, total: u64) -> u32 {
    // SAFETY: sweep_with_callback keeps this stack owner alive for the
    // synchronous foreign call; the library never retains or calls it later.
    let state = unsafe { &mut *user.cast::<ObserverCall<'_>>() };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        (state.callback)(completed, total)
    })) {
        Ok(keep_running) => u32::from(keep_running),
        Err(_) => {
            state.panicked = true;
            0
        }
    }
}

type CloseFn = unsafe extern "C" fn(*mut c_void);

/// Shared NK context interface; extending NK_ABI 1 is capability-checked.
const CONTEXT_ABI: u32 = 2;
#[repr(C)]
#[derive(Clone, Copy)]
struct NkWord {
    address: u64,
    value: u32,
    reserved: u32,
}
const _: () = assert!(std::mem::size_of::<NkWord>() == 16);
type MainRunFn = unsafe extern "C" fn(
    *mut c_void,
    *const NkWord,
    u32,
    *mut u32,
    *mut u32,
    *mut u32,
    *mut c_char,
    u32,
) -> i32;

type MainComponentsFn = unsafe extern "C" fn(
    *mut c_void, *const NkWord, u32, *mut u32, *mut u32, *mut u32,
    *mut c_char, u32, *mut crate::damage::DamageWords,
) -> i32;

type MainTraceFn = unsafe extern "C" fn(
    *mut c_void,
    *const NkWord,
    u32,
    *mut u32,
    *mut u32,
    *mut u32,
    *mut c_char,
    u32,
    *mut u64,
    u32,
    *mut u32,
) -> i32;

#[derive(Debug, Clone, serde::Serialize)]
pub struct MainCpuFailure {
    pub message: String,
    pub bail: u32,
    pub bail_rva: u32,
}
impl std::fmt::Display for MainCpuFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (bail {}, RVA {:#x})",
            self.message, self.bail, self.bail_rva
        )
    }
}
impl std::error::Error for MainCpuFailure {}

fn require_context_abi(
    lib: &os::Library,
    context: &EncodedContext,
    device_packets: bool,
) -> Result<(), String> {
    let symbol = lib
        .symbol(b"nk_context_abi\0")
        .ok_or("kernel does not support captured scene contexts")?;
    // SAFETY: versioned, argument-less capability query exported by the kernel.
    let abi = unsafe { std::mem::transmute::<*mut c_void, AbiFn>(symbol)() };
    if abi != CONTEXT_ABI {
        return Err(format!(
            "kernel context interface {abi} is not {CONTEXT_ABI}"
        ));
    }
    let bytes = context.file("frame.bin").ok_or("captured frame missing")?;
    let frame = crate::pipeline::Frame::load(bytes).map_err(str::to_owned)?;
    // CPU MAIN receives the already composed, ordered packet words from the
    // pipeline. Only the device search ABI consumes packet policies itself.
    if device_packets && frame.hit_packets.iter().any(|packet| {
        matches!(
            packet,
            crate::pipeline::HitPacket::UnimbuedOwnerProjectile { .. }
        )
    }) {
        let symbol = lib
            .symbol(b"nk_hit_packet_abi\0")
            .ok_or("kernel does not apply per-hit packet inputs")?;
        // SAFETY: the optional capability export is an argument-less C query.
        let version = unsafe { std::mem::transmute::<*mut c_void, AbiFn>(symbol)() };
        if version != 3 {
            return Err(format!("unsupported hit packet interface {version}"));
        }
    }
    Ok(())
}

fn validate_product_contract(features: &[&str], version: Option<u32>) -> Result<(), String> {
    if features.contains(&"source_rebuild") {
        return Err("this GPU search ABI does not execute the candidate source rebuild program; use the full-MAIN CPU fallback".into());
    }
    if features.is_empty() || version == Some(PRODUCT_CONTRACT_VERSION) {
        return Ok(());
    }
    let actual = version.map(|version| version.to_string()).unwrap_or_else(|| "missing".into());
    Err(format!(
        "graphics-card kernel product contract {actual} does not support frame sections {}; revision {PRODUCT_CONTRACT_VERSION} is required; use the full-MAIN CPU fallback or rebuild this kernel",
        features.join(", ")
    ))
}

fn require_product_contract(lib: &os::Library, frame: &[u8]) -> Result<(), String> {
    let features = crate::pipeline::Frame::product_kernel_features(frame).map_err(str::to_owned)?;
    let version = if features.is_empty() {
        None
    } else {
        lib.symbol(b"nk_product_contract_version\0").map(|query| {
            // SAFETY: argument-less C capability query; this does not open or
            // invoke the GPU and is required before any frame reaches nk_open.
            unsafe { std::mem::transmute::<*mut c_void, AbiFn>(query)() }
        })
    };
    validate_product_contract(&features, version)
}

fn captured_main_files(context: &EncodedContext) -> Result<Vec<NkFile>, String> {
    [
        (&b"image.bin\0"[..], "image.bin"),
        (&b"scene_entry.bin\0"[..], "scene_entry.bin"),
        (&b"image_known.bin\0"[..], "image_known.bin"),
        (&b"scene_metadata.bin\0"[..], "scene_metadata.bin"),
    ]
    .into_iter()
    .map(|(name, file)| {
        let bytes = context
            .file(file)
            .ok_or_else(|| format!("captured scene is missing {file}"))?;
        Ok(NkFile {
            name: name.as_ptr().cast(),
            data: bytes.as_ptr(),
            size: bytes.len() as u64,
        })
    })
    .collect()
}

#[repr(C)]
struct NkPreparedContact {
    start: u32,
    count: u32,
}

const _: () = assert!(std::mem::size_of::<NkPreparedContact>() == 8);

type PreparedRunFn = unsafe extern "C" fn(
    *mut c_void,
    *const NkWord,
    u32,
    *const NkPreparedContact,
    u32,
    *mut PreparedDamage,
    *mut c_char,
    u32,
) -> i32;

/// Optional native timing ABI. Durations describe the last actual submission;
/// CUDA events are opt-in and are valid only after successful synchronization.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct NkPreparedProfile {
    abi_version: u32,
    struct_bytes: u32,
    success: u32,
    cuda_events_valid: u32,
    contacts: u64,
    input_words: u64,
    h2d_bytes: u64,
    d2h_bytes: u64,
    allocation_calls: u64,
    allocation_bytes: u64,
    capacity_bytes: u64,
    stream_creations: u64,
    host_pack_ms: f64,
    host_reserve_ms: f64,
    host_enqueue_ms: f64,
    host_wait_ms: f64,
    h2d_ms: f64,
    kernel_ms: f64,
    d2h_ms: f64,
    total_ms: f64,
}
const _: () = assert!(std::mem::size_of::<NkPreparedProfile>() == 144);

type PreparedProfileEnableFn = unsafe extern "C" fn(*mut c_void, u32) -> i32;
type PreparedProfileLastFn = unsafe extern "C" fn(*mut c_void, *mut NkPreparedProfile, u32) -> i32;

#[derive(Clone, Copy)]
struct PreparedProfileApi {
    enable: PreparedProfileEnableFn,
    last: PreparedProfileLastFn,
}
impl PreparedProfileApi {
    fn load(lib: &os::Library) -> Option<Self> {
        let abi = lib.symbol(b"nk_main_device_profile_abi\0")?;
        // SAFETY: the optional query has the argument-less capability signature.
        if unsafe { std::mem::transmute::<*mut c_void, AbiFn>(abi)() } != 1 {
            return None;
        }
        let enable = lib.symbol(b"nk_main_device_profile_enable\0")?;
        let last = lib.symbol(b"nk_main_device_profile_last\0")?;
        // SAFETY: version 1 fixes the synchronous handle and output-buffer ABI.
        Some(unsafe {
            Self {
                enable: std::mem::transmute::<*mut c_void, PreparedProfileEnableFn>(enable),
                last: std::mem::transmute::<*mut c_void, PreparedProfileLastFn>(last),
            }
        })
    }
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct PreparedNativeStats {
    pub profiled_calls: u64,
    pub successful_calls: u64,
    pub event_timed_calls: u64,
    pub profile_read_errors: u64,
    pub contacts: u64,
    pub input_words: u64,
    pub h2d_bytes: u64,
    pub d2h_bytes: u64,
    pub allocation_calls: u64,
    pub allocation_bytes: u64,
    pub peak_capacity_bytes: u64,
    pub stream_creations: u64,
    pub host_pack_ms: f64,
    pub host_reserve_ms: f64,
    pub host_enqueue_ms: f64,
    pub host_wait_ms: f64,
    pub h2d_ms: f64,
    pub kernel_ms: f64,
    pub d2h_ms: f64,
    pub total_ms: f64,
}
impl PreparedNativeStats {
    fn add(&mut self, profile: NkPreparedProfile) {
        self.profiled_calls += 1;
        self.successful_calls += u64::from(profile.success != 0);
        self.contacts += profile.contacts;
        self.input_words += profile.input_words;
        self.h2d_bytes += profile.h2d_bytes;
        self.d2h_bytes += profile.d2h_bytes;
        self.allocation_calls += profile.allocation_calls;
        self.allocation_bytes += profile.allocation_bytes;
        self.peak_capacity_bytes = self.peak_capacity_bytes.max(profile.capacity_bytes);
        self.stream_creations += profile.stream_creations;
        self.host_pack_ms += profile.host_pack_ms;
        self.host_reserve_ms += profile.host_reserve_ms;
        self.host_enqueue_ms += profile.host_enqueue_ms;
        self.host_wait_ms += profile.host_wait_ms;
        self.total_ms += profile.total_ms;
        if profile.cuda_events_valid != 0 {
            self.event_timed_calls += 1;
            self.h2d_ms += profile.h2d_ms;
            self.kernel_ms += profile.kernel_ms;
            self.d2h_ms += profile.d2h_ms;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct PreparedDeviceStats {
    pub reuse: PreparedReuseStats,
    pub rust_pack_ms: f64,
    /// Wall time inside nk_main_device_run_components, including host work.
    pub native_call_ms: f64,
    pub profiling_available: bool,
    pub profiling_enabled: bool,
    pub native: PreparedNativeStats,
}

#[derive(Default)]
struct PreparedPacking {
    words: Vec<NkWord>,
    spans: Vec<NkPreparedContact>,
    scores: Vec<PreparedDamage>,
    rust_pack_ms: f64,
    native_call_ms: f64,
    native_stats: PreparedNativeStats,
}
impl PreparedPacking {
    fn submit(
        &mut self,
        handle: *mut c_void,
        run: PreparedRunFn,
        contacts: &[&[(u64, u32)]],
        profile: Option<PreparedProfileApi>,
    ) -> Result<Vec<PreparedDamage>, String> {
        let packing = Instant::now();
        let count = u32::try_from(contacts.len()).map_err(|_| "too many CUDA MAIN contacts")?;
        let word_count = contacts.iter().try_fold(0usize, |total, contact| total.checked_add(contact.len()))
            .and_then(|total| u32::try_from(total).ok())
            .ok_or("too many CUDA MAIN words")?;
        self.words.clear();
        self.spans.clear();
        self.scores.clear();
        self.words.try_reserve(word_count as usize).map_err(|_| "CUDA MAIN word buffer allocation failed")?;
        self.spans.try_reserve(contacts.len()).map_err(|_| "CUDA MAIN contact buffer allocation failed")?;
        self.scores.try_reserve(contacts.len()).map_err(|_| "CUDA MAIN result buffer allocation failed")?;
        for contact in contacts {
            let start = self.words.len() as u32;
            let count = contact.len() as u32;
            self.spans.push(NkPreparedContact { start, count });
            self.words.extend(contact.iter().map(|&(address, value)| NkWord { address, value, reserved: 0 }));
        }
        let unwritten = PreparedDamage {
            damage: crate::damage::DamageWords { reserved: u32::MAX, ..Default::default() },
            ..Default::default()
        };
        self.scores.resize(contacts.len(), unwritten);
        self.rust_pack_ms += packing.elapsed().as_secs_f64() * 1000.0;
        let mut err = [0u8; ERROR_LEN];
        let native = Instant::now();
        // SAFETY: the owner exclusively borrows its native handle. Every buffer
        // remains alive until the synchronous component ABI returns.
        let fine = unsafe {
            run(handle, self.words.as_ptr(), word_count, self.spans.as_ptr(), count,
                self.scores.as_mut_ptr(), err.as_mut_ptr().cast(), ERROR_LEN as u32)
        };
        self.native_call_ms += native.elapsed().as_secs_f64() * 1000.0;
        if let Some(api) = profile {
            let mut last = NkPreparedProfile::default();
            let bytes = std::mem::size_of::<NkPreparedProfile>() as u32;
            // SAFETY: capability version 1 fixes this 144-byte output record.
            let available = unsafe { (api.last)(handle, &mut last, bytes) };
            if available == 1 && last.abi_version == 1 && last.struct_bytes == bytes {
                self.native_stats.add(last);
            } else {
                self.native_stats.profile_read_errors += 1;
            }
        }
        if fine != 1 {
            return Err(message(&err));
        }
        Ok(self.scores.clone())
    }
}

/// Executes one prepared contact per lane. The source and event producers
/// remain on the host and use the same captured inputs as the CPU oracle.
pub(crate) struct MainDevice {
    handle: *mut c_void,
    run: PreparedRunFn,
    close: CloseFn,
    reuse: prepared_reuse::PreparedReuse,
    packing: PreparedPacking,
    profile: Option<PreparedProfileApi>,
    profiling_enabled: bool,
    _lib: os::Library,
}

impl MainDevice {
    pub(crate) fn open(dir: &Path, context: &EncodedContext) -> Result<Self, String> {
        if !context.is_captured_scene() {
            return Err("prepared CUDA MAIN requires a captured scene".into());
        }
        let path = dir.join(if cfg!(windows) { "main_device.dll" } else { "main_device.so" });
        let lib = os::Library::open(&path).ok_or("prepared CUDA MAIN library is unavailable")?;
        require_context_abi(&lib, context, false)?;
        let abi = lib
            .symbol(b"nk_main_device_components_abi\0")
            .ok_or("prepared CUDA MAIN has no component ABI")?;
        // SAFETY: the version query has the argument-less C signature.
        if unsafe { std::mem::transmute::<*mut c_void, AbiFn>(abi)() } != 1 {
            return Err("unsupported prepared CUDA MAIN component ABI".into());
        }
        let open = lib.symbol(b"nk_main_device_open\0").ok_or("missing CUDA MAIN open")?;
        let run = lib
            .symbol(b"nk_main_device_run_components\0")
            .ok_or("missing CUDA MAIN run")?;
        let close = lib.symbol(b"nk_main_device_close\0").ok_or("missing CUDA MAIN close")?;
        let files = captured_main_files(context)?;
        let mut err = [0u8; ERROR_LEN];
        // SAFETY: ABI 1 fixes these signatures. The captured files remain live
        // until open returns, and native open owns any copies it retains.
        let (handle, run, close) = unsafe {
            (
                std::mem::transmute::<*mut c_void, OpenFn>(open)(
                    files.as_ptr(),
                    files.len() as u32,
                    err.as_mut_ptr().cast(),
                    ERROR_LEN as u32,
                ),
                std::mem::transmute::<*mut c_void, PreparedRunFn>(run),
                std::mem::transmute::<*mut c_void, CloseFn>(close),
            )
        };
        if handle.is_null() {
            return Err(message(&err));
        }
        let profile = PreparedProfileApi::load(&lib);
        Ok(Self {
            handle, run, close,
            // Reuse remains opt-in for workloads with repeated packet bytes.
            reuse: prepared_reuse::PreparedReuse::new(false),
            packing: PreparedPacking::default(),
            profile,
            profiling_enabled: false,
            _lib: lib,
        })
    }

    /// Diagnostic control for exact direct-versus-reused acceptance. Changing
    /// this clears stored answers, and never clears a previous device failure.
    pub(crate) fn set_reuse_enabled(&mut self, enabled: bool) {
        self.reuse.set_enabled(enabled);
    }

    pub(crate) fn reset_statistics(&mut self) {
        self.reuse.reset_statistics();
        self.packing.rust_pack_ms = 0.0;
        self.packing.native_call_ms = 0.0;
        self.packing.native_stats = PreparedNativeStats::default();
    }

    /// False means the loaded older DLL has no optional timing capability.
    pub(crate) fn enable_profiling(&mut self, enabled: bool) -> Result<bool, String> {
        let Some(api) = self.profile else { return Ok(false); };
        // SAFETY: optional timing changes only this exclusively owned handle.
        if unsafe { (api.enable)(self.handle, u32::from(enabled)) } != 1 {
            return Err("prepared CUDA MAIN could not configure profiling".into());
        }
        self.profiling_enabled = enabled;
        Ok(true)
    }

    pub(crate) fn stats(&self) -> PreparedDeviceStats {
        PreparedDeviceStats {
            reuse: self.reuse.stats(),
            rust_pack_ms: self.packing.rust_pack_ms,
            native_call_ms: self.packing.native_call_ms,
            profiling_available: self.profile.is_some(),
            profiling_enabled: self.profiling_enabled,
            native: self.packing.native_stats,
        }
    }

    pub(crate) fn run(
        &mut self,
        contacts: &[Vec<(u64, u32)>],
    ) -> Result<Vec<PreparedDamage>, String> {
        let Self { handle, run, reuse, packing, profile, profiling_enabled, .. } = self;
        let active_profile = if *profiling_enabled { *profile } else { None };
        reuse.execute(contacts, |unique| packing.submit(*handle, *run, unique, active_profile))
    }
}

impl Drop for MainDevice {
    fn drop(&mut self) {
        // SAFETY: close this handle once while its library is still loaded.
        unsafe { (self.close)(self.handle) };
    }
}

/// Complete translated MAIN on CPU, with no CUDA device allocation in its open
/// or run operations. Its scene is immutable; Engine serializes the handle.
pub struct MainCpu {
    handle: *mut c_void,
    run: MainRunFn,
    trace: Option<MainTraceFn>,
    components: Option<MainComponentsFn>,
    close: CloseFn,
    _lib: os::Library,
}
// SAFETY: this owned handle is only used through &mut self and is kept behind
// the Engine's mutex. It is never concurrently accessed or exposed as a pointer.
unsafe impl Send for MainCpu {}
impl MainCpu {
    pub fn open(dir: &Path, context: &EncodedContext) -> Result<Self, String> {
        if !context.is_captured_scene() {
            return Err("full MAIN requires an explicit captured scene".into());
        }
        // The portable adapter exports only the real host MAIN ABI. It is
        // deliberately a different file from the CUDA search library, so a
        // host-only installation cannot advertise a device search capability.
        #[cfg(windows)]
        let path = if dir.join("main_cpu.dll").is_file() {
            dir.join("main_cpu.dll")
        } else { dir.join("kernel.dll") };
        #[cfg(target_os = "linux")]
        let path = dir.join("main_cpu.so");
        #[cfg(not(any(windows, target_os = "linux")))]
        let path = dir.join("main_cpu.unsupported");
        if !path.is_file() {
            return Err("this attack has no full-MAIN CPU library".into());
        }
        let lib =
            os::Library::open(&path).ok_or("the full-MAIN CPU library could not be loaded")?;
        require_context_abi(&lib, context, false)?;
        let get = |name: &[u8]| {
            lib.symbol(name).ok_or_else(|| {
                format!(
                    "kernel lacks {}",
                    String::from_utf8_lossy(&name[..name.len() - 1])
                )
            })
        };
        // SAFETY: each versioned export has the exact shared NK C signature.
        let (open, run, close) = unsafe {
            (
                std::mem::transmute::<*mut c_void, OpenFn>(get(b"nk_main_cpu_open\0")?),
                std::mem::transmute::<*mut c_void, MainRunFn>(get(b"nk_main_cpu_run\0")?),
                std::mem::transmute::<*mut c_void, CloseFn>(get(b"nk_main_cpu_close\0")?),
            )
        };
        let files = captured_main_files(context)?;
        let mut err = [0u8; ERROR_LEN];
        // SAFETY: the fixed input buffers outlive the call; open copies what it keeps.
        let handle = unsafe {
            open(
                files.as_ptr(),
                files.len() as u32,
                err.as_mut_ptr().cast(),
                ERROR_LEN as u32,
            )
        };
        if handle.is_null() {
            return Err(message(&err));
        }
        // SAFETY: this optional export adds the read buffer to the C run ABI.
        let trace = lib
            .symbol(b"nk_main_cpu_run_trace\0")
            .map(|symbol| unsafe { std::mem::transmute::<*mut c_void, MainTraceFn>(symbol) });
        let components = lib.symbol(b"nk_main_cpu_components_abi\0").and_then(|query| {
            // SAFETY: optional ABI query has the same argument-less C layout.
            let version=unsafe { std::mem::transmute::<*mut c_void, AbiFn>(query)() };
            (version==1).then(|| lib.symbol(b"nk_main_cpu_run_components\0")).flatten()
        }).map(|symbol| {
            // SAFETY: component ABI1 appends one aligned16-byte output buffer.
            unsafe { std::mem::transmute::<*mut c_void,MainComponentsFn>(symbol) }
        });
        Ok(Self {
            handle,
            run,
            trace,
            components,
            close,
            _lib: lib,
        })
    }

    pub fn run(&mut self, words: &[(u64, u32)]) -> Result<u32, MainCpuFailure> {
        self.run_with_reads(words, None).map(|(bits, _)| bits)
    }

    pub fn has_components(&self) -> bool { self.components.is_some() }

    pub fn run_components(&mut self, words: &[(u64,u32)]) -> Result<crate::damage::DamageWords,MainCpuFailure> {
        let run=self.components.ok_or_else(|| MainCpuFailure { message:"MAIN component ABI1 is unavailable".into(),bail:0,bail_rva:0 })?;
        let n=u32::try_from(words.len()).map_err(|_| MainCpuFailure { message:"too many MAIN words".into(),bail:0,bail_rva:0 })?;
        let words:Vec<NkWord>=words.iter().map(|&(address,value)| NkWord { address,value,reserved:0 }).collect();
        let (mut bits,mut bail,mut bail_rva)=(0,0,0);
        let mut output=crate::damage::DamageWords::default();let mut err=[0u8;ERROR_LEN];
        // SAFETY: live exclusive handle, checked C word layout and correctly sized
        // output/error buffers; no device allocation or game process is involved.
        let fine=unsafe { run(self.handle,words.as_ptr(),n,&mut bits,&mut bail,&mut bail_rva,
            err.as_mut_ptr().cast(),ERROR_LEN as u32,&mut output) };
        if fine!=1 || bail!=0 || output.reserved!=0 || output.total!=bits {
            return Err(MainCpuFailure { message:message(&err),bail,bail_rva });
        }
        Ok(output)
    }

    pub(crate) fn has_read_trace(&self) -> bool {
        self.trace.is_some()
    }

    pub(crate) fn run_with_reads(
        &mut self,
        words: &[(u64, u32)],
        reads: Option<&mut [u64]>,
    ) -> Result<(u32, Option<usize>), MainCpuFailure> {
        let count = u32::try_from(words.len()).map_err(|_| MainCpuFailure {
            message: "too many MAIN words".into(),
            bail: 0,
            bail_rva: 0,
        })?;
        let words: Vec<NkWord> = words
            .iter()
            .map(|&(address, value)| NkWord {
                address,
                value,
                reserved: 0,
            })
            .collect();
        let (mut bits, mut bail, mut bail_rva) = (0, 0, 0);
        let mut err = [0u8; ERROR_LEN];
        // SAFETY: the handle is live, words use the checked 16-byte C layout,
        // and every output/error pointer refers to a correctly sized buffer.
        let (fine, observed) = unsafe {
            if let (Some(trace), Some(reads)) = (self.trace, reads) {
                let mut observed = u32::MAX;
                let capacity = reads.len() as u32;
                let fine = trace(
                    self.handle,
                    words.as_ptr(),
                    count,
                    &mut bits,
                    &mut bail,
                    &mut bail_rva,
                    err.as_mut_ptr().cast(),
                    ERROR_LEN as u32,
                    reads.as_mut_ptr(),
                    capacity,
                    &mut observed,
                );
                (fine, (observed <= capacity).then_some(observed as usize))
            } else {
                (
                    (self.run)(
                        self.handle,
                        words.as_ptr(),
                        count,
                        &mut bits,
                        &mut bail,
                        &mut bail_rva,
                        err.as_mut_ptr().cast(),
                        ERROR_LEN as u32,
                    ),
                    None,
                )
            }
        };
        if fine != 1 || bail != 0 {
            return Err(MainCpuFailure {
                message: message(&err),
                bail,
                bail_rva,
            });
        }
        Ok((bits, observed))
    }
}
impl Drop for MainCpu {
    fn drop(&mut self) {
        // SAFETY: close the owned handle once while its library remains loaded.
        unsafe { (self.close)(self.handle) };
    }
}

#[cfg(windows)]
mod os {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
        fn FreeLibrary(module: *mut c_void) -> i32;
    }
    const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x8;

    pub struct Library(*mut c_void);

    impl Library {
        pub fn open(path: &std::path::Path) -> Option<Library> {
            use std::os::windows::ffi::OsStrExt;
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
            // SAFETY: a NUL-terminated wide path; the module stays loaded until Drop frees it.
            let m = unsafe {
                LoadLibraryExW(
                    wide.as_ptr(),
                    std::ptr::null_mut(),
                    LOAD_WITH_ALTERED_SEARCH_PATH,
                )
            };
            (!m.is_null()).then_some(Library(m))
        }

        pub fn symbol(&self, name: &[u8]) -> Option<*mut c_void> {
            debug_assert_eq!(name.last(), Some(&0));
            // SAFETY: a live module and a NUL-terminated name.
            let p = unsafe { GetProcAddress(self.0, name.as_ptr()) };
            (!p.is_null()).then_some(p)
        }
    }

    impl Drop for Library {
        fn drop(&mut self) {
            // SAFETY: the module came from LoadLibraryExW and every handle into it is closed first (Gpu::drop).
            unsafe { FreeLibrary(self.0) };
        }
    }
}

#[cfg(target_os = "linux")]
mod os {
    use std::ffi::{c_char, c_int, c_void, CString};
    use std::os::unix::ffi::OsStrExt;

    #[link(name = "dl")]
    extern "C" {
        fn dlopen(path: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> c_int;
    }
    const RTLD_NOW: c_int = 2;

    pub struct Library(*mut c_void);
    impl Library {
        pub fn open(path: &std::path::Path) -> Option<Library> {
            // A full, canonical filename avoids resolving a candidate library
            // from the process working directory or the dynamic search path.
            let absolute = path.canonicalize().ok()?;
            let name = CString::new(absolute.as_os_str().as_bytes()).ok()?;
            // SAFETY: NUL-terminated path; RTLD_NOW validates dependencies now;
            // the owning Library keeps all symbol pointers alive until Drop.
            let handle = unsafe { dlopen(name.as_ptr(), RTLD_NOW) };
            (!handle.is_null()).then_some(Library(handle))
        }
        pub fn symbol(&self, name: &[u8]) -> Option<*mut c_void> {
            debug_assert_eq!(name.last(), Some(&0));
            // SAFETY: live owned handle and NUL-terminated static export name.
            let pointer = unsafe { dlsym(self.0, name.as_ptr().cast()) };
            (!pointer.is_null()).then_some(pointer)
        }
    }
    impl Drop for Library {
        fn drop(&mut self) {
            // SAFETY: the native handle has already been closed by its owner's
            // Drop before this last library field is released.
            unsafe { dlclose(self.0) };
        }
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod os {
    pub struct Library;
    impl Library {
        pub fn open(_: &std::path::Path) -> Option<Library> {
            None
        }
        pub fn symbol(&self, _: &[u8]) -> Option<*mut std::ffi::c_void> {
            None
        }
    }
}

/// An opened kernel with the backpack it last received.
pub struct Gpu {
    handle: *mut c_void,
    set_pool: PoolFn,
    cases: CasesFn,
    search: SearchFn,
    observed_search: Option<ObservedSearchFn>,
    set_predicate: Option<SetPredicate>,
    close: CloseFn,
    pool: Option<Vec<u8>>,
    _lib: os::Library,
}

// SAFETY: the library serializes every call on its handle with its own mutex, and the engine keeps the Gpu behind a
// Mutex as well; the raw handle is never shared outside these calls.
unsafe impl Send for Gpu {}

fn message(buffer: &[u8]) -> String {
    let end = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
    String::from_utf8_lossy(&buffer[..end]).into_owned()
}

fn limits(l: &Limits) -> NkLimits {
    NkLimits {
        on: 1,
        max_load: l.max_load,
        min_defense: l.min_defense,
        min_health: l.min_health,
    }
}

impl Gpu {
    /// Load `kernel.dll` from the bundle directory and hand it the bundle's files. `dom_base` and `dom_known` are the
    /// formula's baseline domain, which programs.txt holds and the library reads as files.
    pub fn open(dir: &Path, dom_base: &[u8], dom_known: &[u8]) -> Result<Gpu, String> {
        Self::open_inputs(dir, dom_base, dom_known, None, None)
    }

    /// NK_ABI 1 accepts named in-memory files: replace its context files with
    /// exactly the buffers that the CPU parsed, without rewriting a bundle.
    pub fn open_with_context(
        dir: &Path,
        dom_base: &[u8],
        dom_known: &[u8],
        context: &EncodedContext,
    ) -> Result<Gpu, String> {
        Self::open_inputs(dir, dom_base, dom_known, Some(context), None)
    }

    pub(crate) fn open_with_inputs(
        dir: &Path, dom_base: &[u8], dom_known: &[u8], context: Option<&EncodedContext>,
        inputs: &crate::bundle_inputs::BundleInputs,
    ) -> Result<Gpu, String> {
        Self::open_inputs(dir, dom_base, dom_known, context, Some(inputs))
    }

    fn open_inputs(
        dir: &Path,
        dom_base: &[u8],
        dom_known: &[u8],
        context: Option<&EncodedContext>,
        inputs: Option<&crate::bundle_inputs::BundleInputs>,
    ) -> Result<Gpu, String> {
        let path = dir.join("kernel.dll");
        if !path.is_file() {
            return Err("this attack has no graphics-card kernel".into());
        }
        let lib = os::Library::open(&path).ok_or("the graphics-card kernel could not be loaded")?;
        let get = |name: &[u8]| {
            lib.symbol(name).ok_or_else(|| {
                format!(
                    "kernel lacks {}",
                    String::from_utf8_lossy(&name[..name.len() - 1])
                )
            })
        };
        // SAFETY: each symbol is the library's export of that name, declared with the same C signature there.
        let (abi, item, slots, open, set_pool, cases, search, close) = unsafe {
            (
                std::mem::transmute::<*mut c_void, AbiFn>(get(b"nk_abi\0")?),
                std::mem::transmute::<*mut c_void, AbiFn>(get(b"nk_item_size\0")?),
                std::mem::transmute::<*mut c_void, AbiFn>(get(b"nk_cand_slots\0")?),
                std::mem::transmute::<*mut c_void, OpenFn>(get(b"nk_open\0")?),
                std::mem::transmute::<*mut c_void, PoolFn>(get(b"nk_set_pool\0")?),
                std::mem::transmute::<*mut c_void, CasesFn>(get(b"nk_cases\0")?),
                std::mem::transmute::<*mut c_void, SearchFn>(get(b"nk_search\0")?),
                std::mem::transmute::<*mut c_void, CloseFn>(get(b"nk_close\0")?),
            )
        };
        let observed_search = match lib.symbol(b"nk_search_observer_abi\0") {
            Some(query) => {
                // SAFETY: this optional export is an argument-less capability query.
                let version = unsafe { std::mem::transmute::<*mut c_void, AbiFn>(query)() };
                if version == 1 {
                    lib.symbol(b"nk_search_observed\0").map(|function| {
                        // SAFETY: observer ABI 1 declares this exact blocking signature.
                        unsafe { std::mem::transmute::<*mut c_void, ObservedSearchFn>(function) }
                    })
                } else {
                    None
                }
            }
            None => None,
        };
        let set_predicate = SetPredicate::load(&lib);
        // SAFETY: argument-less functions.
        let (a, i, s) = unsafe { (abi(), item(), slots()) };
        if a != ABI || i as usize != ITEM || s as usize != SLOTS {
            return Err(format!(
                "kernel interface {a}/{i}/{s} is not {ABI}/{ITEM}/{SLOTS}"
            ));
        }
        if let Some(context) = context.filter(|context| context.is_captured_scene()) {
            require_context_abi(&lib, context, true)?;
        }
        let mut named: Vec<(&'static [u8], Cow<'_, [u8]>)> = Vec::new();
        for (name, file) in [
            (&b"tables.bin\0"[..], "tables.bin"),
            (b"frame.bin\0", "frame.bin"),
            (b"characters.bin\0", "characters.bin"),
            (b"hits.txt\0", "hits.txt"),
            (b"arena.bin\0", "arena.bin"),
            (b"mapped.bin\0", "mapped.bin"),
            (b"gpu_setup.txt\0", "gpu_setup.txt"),
        ] {
            let bytes = match context.and_then(|c| c.file(file)) {
                Some(bytes) => Cow::Borrowed(bytes),
                None if inputs.is_some() => Cow::Borrowed(inputs.and_then(|i| i.file(file))
                    .ok_or_else(|| format!("immutable bundle snapshot lacks {file}"))?),
                None => {
                    Cow::Owned(std::fs::read(dir.join(file)).map_err(|e| format!("{file}: {e}"))?)
                }
            };
            named.push((name, bytes));
        }
        let actual_frame = named.iter().find(|(name, _)| *name == b"frame.bin\0")
            .map(|(_, bytes)| bytes.as_ref()).ok_or("GPU frame missing")?;
        require_product_contract(&lib, actual_frame)?;
        // CapturedScene always supplies its immutable image and entry together.
        // RecordedScenario retains the old optional bundle-image behavior.
        if let Some(image) = context.and_then(|c| c.file("image.bin")) {
            named.push((b"image.bin\0", Cow::Borrowed(image)));
        } else if let Some(inputs) = inputs {
            if let Some(image) = inputs.file("image.bin") {
                named.push((b"image.bin\0", Cow::Borrowed(image)));
            }
        } else {
            let image = dir.join("image.bin");
            if image.is_file() {
                named.push((
                    b"image.bin\0",
                    Cow::Owned(std::fs::read(&image).map_err(|e| format!("image.bin: {e}"))?),
                ));
            }
        }
        if let Some(entry) = context.and_then(|c| c.file("scene_entry.bin")) {
            named.push((b"scene_entry.bin\0", Cow::Borrowed(entry)));
        }
        for (name, file) in [
            (&b"image_known.bin\0"[..], "image_known.bin"),
            (&b"scene_metadata.bin\0"[..], "scene_metadata.bin"),
        ] {
            if let Some(bytes) = context.and_then(|c| c.file(file)) {
                named.push((name, Cow::Borrowed(bytes)));
            }
        }
        named.push((b"dom_base.bin\0", Cow::Borrowed(dom_base)));
        named.push((b"dom_known.bin\0", Cow::Borrowed(dom_known)));
        let files: Vec<NkFile> = named
            .iter()
            .map(|(n, b)| NkFile {
                name: n.as_ptr().cast(),
                data: b.as_ptr(),
                size: b.len() as u64,
            })
            .collect();
        let mut err = [0u8; ERROR_LEN];
        // SAFETY: the file table and its buffers outlive the call; the library copies what it keeps.
        let handle = unsafe {
            open(
                files.as_ptr(),
                files.len() as u32,
                err.as_mut_ptr().cast(),
                ERROR_LEN as u32,
            )
        };
        if handle.is_null() {
            return Err(message(&err));
        }
        Ok(Gpu {
            handle,
            set_pool,
            cases,
            search,
            observed_search,
            set_predicate,
            close,
            pool: None,
            _lib: lib,
        })
    }

    /// Give the card this backpack, unless it already holds the same rows.
    pub fn use_pool(&mut self, pool: &[[u8; ITEM]]) -> Result<(), String> {
        let bytes = pool.as_flattened();
        if self.pool.as_deref() == Some(bytes) {
            return Ok(());
        }
        self.pool = None;
        let mut err = [0u8; ERROR_LEN];
        // SAFETY: a live handle and a buffer of whole item records.
        let ok = unsafe {
            (self.set_pool)(
                self.handle,
                bytes.as_ptr(),
                bytes.len() as u64,
                err.as_mut_ptr().cast(),
                ERROR_LEN as u32,
            )
        };
        if ok == 0 {
            return Err(message(&err));
        }
        self.pool = Some(bytes.to_vec());
        Ok(())
    }

    pub fn cases(&mut self, list: &[Candidate], l: &Limits) -> Result<Vec<CardOutcome>, String> {
        self.cases_with_sets(list, l, &[])
    }

    /// Exact native set admission precedes scoring and uses OutsideLimits for
    /// excluded candidates. Empty rules retain the legacy NK_ABI 1 call.
    pub fn cases_with_sets(
        &mut self,
        list: &[Candidate],
        l: &Limits,
        sets: &[crate::search::SetRule],
    ) -> Result<Vec<CardOutcome>, String> {
        let predicate = if sets.is_empty() {
            None
        } else {
            Some(
                self.set_predicate
                    .ok_or("kernel lacks native set-predicate capability")?,
            )
        };
        let set_input = SetInput::new(sets)?;
        let set_rules = set_input.descriptor()?;
        let cand: Vec<NkCand> = list
            .iter()
            .map(|c| NkCand {
                attrs: c.attrs,
                slot: c.items.map(|r| r.map_or(-1, |r| r as i32)),
            })
            .collect();
        let (mut bits, mut ok) = (vec![0u32; list.len()], vec![0u8; list.len()]);
        let lim = limits(l);
        let mut err = [0u8; ERROR_LEN];
        // SAFETY: input/output buffers and both immutable rule arrays remain
        // alive until this synchronous, capability-checked C call returns.
        let fine = unsafe {
            if let Some(predicate) = predicate {
                (predicate.cases)(
                    self.handle,
                    cand.as_ptr(),
                    cand.len() as u64,
                    &lim,
                    &set_rules,
                    bits.as_mut_ptr(),
                    ok.as_mut_ptr(),
                    err.as_mut_ptr().cast(),
                    ERROR_LEN as u32,
                )
            } else {
                (self.cases)(
                    self.handle,
                    cand.as_ptr(),
                    cand.len() as u64,
                    &lim,
                    bits.as_mut_ptr(),
                    ok.as_mut_ptr(),
                    err.as_mut_ptr().cast(),
                    ERROR_LEN as u32,
                )
            }
        };
        if fine == 0 {
            return Err(message(&err));
        }
        ok.iter()
            .zip(bits)
            .map(|(&o, b)| match o {
                0 => Ok(CardOutcome::Refused),
                1 => Ok(CardOutcome::Damage(b)),
                2 => Ok(CardOutcome::NoPath),
                3 => Ok(CardOutcome::OutsideLimits),
                other => Err(format!("kernel returned outcome {other}")),
            })
            .collect()
    }

    pub fn supports_observed_search(&self) -> bool {
        self.observed_search.is_some()
    }

    /// All constrained entry points must be present at the supported version;
    /// legacy, unknown-version, and partially replaced libraries return false.
    pub fn supports_set_constraints(&self) -> bool {
        self.set_predicate.is_some()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sweep(
        &mut self,
        spaces: &[NkSpace],
        lists: &[u32],
        attrs: &[i32; 7],
        l: &Limits,
        keep: f32,
        capacity: usize,
        miss_capacity: usize,
    ) -> Result<Sweep, String> {
        self.sweep_with_sets(spaces, lists, attrs, l, &[], keep, capacity, miss_capacity)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sweep_with_sets(
        &mut self,
        spaces: &[NkSpace],
        lists: &[u32],
        attrs: &[i32; 7],
        l: &Limits,
        sets: &[crate::search::SetRule],
        keep: f32,
        capacity: usize,
        miss_capacity: usize,
    ) -> Result<Sweep, String> {
        self.sweep_with_callback(
            spaces,
            lists,
            attrs,
            l,
            sets,
            keep,
            capacity,
            miss_capacity,
            None,
        )
        .map_err(|error| error.to_string())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sweep_observed(
        &mut self,
        spaces: &[NkSpace],
        lists: &[u32],
        attrs: &[i32; 7],
        l: &Limits,
        keep: f32,
        capacity: usize,
        miss_capacity: usize,
        observer: &mut dyn FnMut(u64, u64) -> bool,
    ) -> Result<Sweep, GpuSweepError> {
        self.sweep_observed_with_sets(
            spaces,
            lists,
            attrs,
            l,
            &[],
            keep,
            capacity,
            miss_capacity,
            observer,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sweep_observed_with_sets(
        &mut self,
        spaces: &[NkSpace],
        lists: &[u32],
        attrs: &[i32; 7],
        l: &Limits,
        sets: &[crate::search::SetRule],
        keep: f32,
        capacity: usize,
        miss_capacity: usize,
        observer: &mut dyn FnMut(u64, u64) -> bool,
    ) -> Result<Sweep, GpuSweepError> {
        self.sweep_with_callback(
            spaces,
            lists,
            attrs,
            l,
            sets,
            keep,
            capacity,
            miss_capacity,
            Some(observer),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn sweep_with_callback(
        &mut self,
        spaces: &[NkSpace],
        lists: &[u32],
        attrs: &[i32; 7],
        l: &Limits,
        sets: &[crate::search::SetRule],
        keep: f32,
        capacity: usize,
        miss_capacity: usize,
        observer: Option<&mut dyn FnMut(u64, u64) -> bool>,
    ) -> Result<Sweep, GpuSweepError> {
        let predicate = if sets.is_empty() {
            None
        } else {
            Some(self.set_predicate.ok_or_else(|| {
                GpuSweepError::Failure("kernel lacks native set-predicate capability".into())
            })?)
        };
        let set_input = SetInput::new(sets).map_err(GpuSweepError::Failure)?;
        let set_rules = set_input.descriptor().map_err(GpuSweepError::Failure)?;
        let mut hits = vec![NkHit::default(); capacity];
        let mut missed = vec![0u64; miss_capacity];
        let (mut hit_n, mut miss_n, mut counts) = (0u32, 0u32, [0u64; 4]);
        let lim = limits(l);
        let mut err = [0u8; ERROR_LEN];
        let fine = if let Some(callback) = observer {
            let mut call = ObserverCall {
                callback,
                panicked: false,
            };
            // SAFETY: all buffers and the borrowed callback owner remain alive
            // until this synchronous, capability-checked C call returns.
            let result = unsafe {
                if let Some(predicate) = predicate {
                    (predicate.observed_search)(
                        self.handle,
                        spaces.as_ptr(),
                        spaces.len() as u32,
                        lists.as_ptr(),
                        lists.len() as u64,
                        attrs.as_ptr(),
                        &lim,
                        &set_rules,
                        keep,
                        0,
                        hits.as_mut_ptr(),
                        capacity as u32,
                        &mut hit_n,
                        counts.as_mut_ptr(),
                        missed.as_mut_ptr(),
                        miss_capacity as u32,
                        &mut miss_n,
                        Some(observe_search),
                        (&mut call as *mut ObserverCall<'_>).cast(),
                        err.as_mut_ptr().cast(),
                        ERROR_LEN as u32,
                    )
                } else {
                    let function = self.observed_search.ok_or_else(|| {
                        GpuSweepError::Failure("kernel lacks observed search capability".into())
                    })?;
                    function(
                        self.handle,
                        spaces.as_ptr(),
                        spaces.len() as u32,
                        lists.as_ptr(),
                        lists.len() as u64,
                        attrs.as_ptr(),
                        &lim,
                        keep,
                        0,
                        hits.as_mut_ptr(),
                        capacity as u32,
                        &mut hit_n,
                        counts.as_mut_ptr(),
                        missed.as_mut_ptr(),
                        miss_capacity as u32,
                        &mut miss_n,
                        Some(observe_search),
                        (&mut call as *mut ObserverCall<'_>).cast(),
                        err.as_mut_ptr().cast(),
                        ERROR_LEN as u32,
                    )
                }
            };
            if call.panicked {
                return Err(GpuSweepError::ObserverFailed);
            }
            if result == 2 {
                return Err(GpuSweepError::Cancelled);
            }
            result
        } else {
            // SAFETY: the matching ABI and all immutable arrays/out buffers
            // outlive the synchronous call; no cancel word is shared.
            unsafe {
                if let Some(predicate) = predicate {
                    (predicate.search)(
                        self.handle,
                        spaces.as_ptr(),
                        spaces.len() as u32,
                        lists.as_ptr(),
                        lists.len() as u64,
                        attrs.as_ptr(),
                        &lim,
                        &set_rules,
                        keep,
                        0,
                        hits.as_mut_ptr(),
                        capacity as u32,
                        &mut hit_n,
                        counts.as_mut_ptr(),
                        missed.as_mut_ptr(),
                        miss_capacity as u32,
                        &mut miss_n,
                        std::ptr::null(),
                        err.as_mut_ptr().cast(),
                        ERROR_LEN as u32,
                    )
                } else {
                    (self.search)(
                        self.handle,
                        spaces.as_ptr(),
                        spaces.len() as u32,
                        lists.as_ptr(),
                        lists.len() as u64,
                        attrs.as_ptr(),
                        &lim,
                        keep,
                        0,
                        hits.as_mut_ptr(),
                        capacity as u32,
                        &mut hit_n,
                        counts.as_mut_ptr(),
                        missed.as_mut_ptr(),
                        miss_capacity as u32,
                        &mut miss_n,
                        std::ptr::null(),
                        err.as_mut_ptr().cast(),
                        ERROR_LEN as u32,
                    )
                }
            }
        };
        match fine {
            1 => {}
            0 => return Err(GpuSweepError::Failure(message(&err))),
            other => {
                return Err(GpuSweepError::Failure(format!(
                    "kernel returned search status {other}"
                )))
            }
        }
        hits.truncate((hit_n as usize).min(capacity));
        missed.truncate((miss_n as usize).min(miss_capacity));
        Ok(Sweep {
            counts,
            hits,
            hit_total: hit_n as u64,
            missed,
        })
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        // SAFETY: the handle came from nk_open and is closed once, before the library unloads.
        unsafe { (self.close)(self.handle) };
    }
}

#[cfg(test)]
mod product_kernel_contract_tests {
    use super::{validate_product_contract, PRODUCT_CONTRACT_VERSION};

    #[test]
    fn legacy_contract_only_accepts_frames_without_new_sections() {
        assert!(validate_product_contract(&[], None).is_ok());
        assert!(validate_product_contract(&[], Some(1)).is_ok());
        for feature in ["owner_resource", "native_sources", "actor_agility"] {
            let error = validate_product_contract(&[feature], None).unwrap_err();
            assert!(error.contains(feature));
            assert!(error.contains("20261115"));
            assert!(error.contains("full-MAIN CPU fallback"));
        }
    }

    #[test]
    fn product_contract_requires_exact_revision_for_new_frame_inputs() {
        let features = ["owner_resource", "native_sources", "actor_agility"];
        assert!(validate_product_contract(&features, Some(PRODUCT_CONTRACT_VERSION)).is_ok());
        for revision in [0, 1, PRODUCT_CONTRACT_VERSION - 1, PRODUCT_CONTRACT_VERSION + 1] {
            assert!(validate_product_contract(&features, Some(revision)).is_err());
        }
    }

    #[test]
    fn dynamic_source_program_never_enters_a_flat_source_device_kernel() {
        for version in [None, Some(PRODUCT_CONTRACT_VERSION), Some(u32::MAX)] {
            let error = validate_product_contract(&["source_rebuild"], version).unwrap_err();
            assert!(error.contains("source rebuild"));
            assert!(error.contains("full-MAIN CPU fallback"));
        }
    }
}
