//! Typed ABI bindings for the shipped `nioh3_seed_accelerator.dll`.
//!
//! This mirrors `nioh3_scroll_editor/seed_accelerator.py`. The same optional
//! module is loaded, the same ABI version and strict-GPU execution policy are
//! enforced at load time, and the same bounded ranges, packed row widths,
//! failure codes and cursor semantics are reproduced. The accelerator does the
//! seed enumeration; the Rust side only marshals bounded chunks and verifies
//! what came back.
//!
//! The library is deliberately never unloaded, matching the Python module's
//! cached `ctypes.WinDLL`, so no in-flight call can race a library unload. All
//! `unsafe` in this file is contained in the private [`platform`] module and in
//! the thin wrappers that call it.

use std::path::Path;
use std::rc::Rc;

/// ABI version the product accepts (`SEED_ACCELERATOR_ABI_VERSION`).
pub const SEED_ACCELERATOR_ABI_VERSION: i32 = 2;
/// `EXECUTION_POLICY_STRICT_GPU`.
pub const EXECUTION_POLICY_STRICT_GPU: i32 = 0;
/// `EXECUTION_POLICY_ALLOW_BULK_CPU`.
pub const EXECUTION_POLICY_ALLOW_BULK_CPU: i32 = 1;

/// `ERROR_RESULT`: every failing `collect_*` export returns this count.
pub const ERROR_RESULT: u64 = u64::MAX;

/// Maximum trials one plain natural-pivot call may scan (Python cap).
pub const MAX_NATURAL_TRIALS: u64 = 1_000_000;
/// Maximum trials one R4 primary-pivot call may scan (native ABI cap).
pub const MAX_R4_PRIMARY_TRIALS: u64 = 50_000_000;
/// Result capacity one R4 primary-pivot call reserves.
pub const MAX_R4_PRIMARY_CAPACITY: u64 = 4_000_000;
/// Maximum trials one fused auxiliary call may scan (native ABI cap).
pub const MAX_AUXILIARY_TRIALS: u64 = 8_000_000;
/// Maximum seeds one per-seed batch predicate call may take (native ABI cap).
pub const MAX_PREDICATE_BATCH: usize = 1_000_000;
/// Maximum criterion groups the native matchers accept.
pub const MAX_CRITERION_GROUPS: usize = 32;
/// Maximum enemy rows the native matcher accepts.
pub const MAX_ENEMY_ROWS: usize = 512;
/// Maximum special-rule rows the native matcher accepts.
pub const MAX_RULE_ROWS: usize = 1024;
/// Every lookup table is 65,536 entries wide.
pub const LOOKUP_ENTRIES: usize = 0x1_0000;

/// Packed `EnemyCandidateInput` width.
pub const ENEMY_ROW_BYTES: usize = 18;
/// Packed `EnemyTerrainInput` width.
pub const TERRAIN_ROW_BYTES: usize = 5;
/// Packed `EnemyContextInput` width.
pub const CONTEXT_ROW_BYTES: usize = 22;
/// Packed `SpecialRuleInput` width.
pub const RULE_ROW_BYTES: usize = 16;

/// The execution policy a caller pins for one operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPolicy {
    /// `EXECUTION_POLICY_STRICT_GPU`: CUDA or nothing.
    StrictGpu,
    /// `EXECUTION_POLICY_ALLOW_BULK_CPU`: explicit opt-in to the bulk CPU path.
    AllowBulkCpu,
}

impl ExecutionPolicy {
    /// The raw `seed_accelerator_set_execution_policy` value.
    pub fn raw(self) -> i32 {
        match self {
            ExecutionPolicy::StrictGpu => EXECUTION_POLICY_STRICT_GPU,
            ExecutionPolicy::AllowBulkCpu => EXECUTION_POLICY_ALLOW_BULK_CPU,
        }
    }

    /// The policy a search request asks for, from `allow_cpu_fallback`.
    pub fn from_allow_cpu_fallback(allow_cpu_fallback: bool) -> Self {
        if allow_cpu_fallback {
            ExecutionPolicy::AllowBulkCpu
        } else {
            ExecutionPolicy::StrictGpu
        }
    }
}

/// Which accelerator path served the last native call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeBackend {
    /// The CUDA kernels ran.
    Cuda,
    /// The native bulk CPU fallback ran under an explicit opt-in.
    NativeCpu,
    /// CUDA was unusable and the strict-GPU policy refused the bulk CPU path.
    CpuBlocked,
    /// No call has been made by this loaded library yet.
    NotUsed,
}

impl NativeBackend {
    /// Decode `seed_accelerator_last_backend`.
    pub fn from_raw(raw: i32) -> Self {
        match raw {
            1 => NativeBackend::Cuda,
            0 => NativeBackend::NativeCpu,
            -2 => NativeBackend::CpuBlocked,
            _ => NativeBackend::NotUsed,
        }
    }
}

/// The CUDA failure stage `seed_accelerator_last_cuda_stage` reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CudaFailureStage {
    DeviceDiscovery,
    SelfTestLaunch,
    SelfTestSynchronize,
    R4PivotAllocation,
    R4PivotUpload,
    R4PivotLaunch,
    R4PivotSynchronize,
    R4PivotCountDownload,
    R4PivotOutputDownload,
    R4PivotCapacity,
    /// A stage this build does not know.
    Unknown(i32),
}

impl CudaFailureStage {
    /// Decode the native stage number.
    pub fn from_raw(raw: i32) -> Self {
        match raw {
            1 => CudaFailureStage::DeviceDiscovery,
            2 => CudaFailureStage::SelfTestLaunch,
            3 => CudaFailureStage::SelfTestSynchronize,
            10 => CudaFailureStage::R4PivotAllocation,
            11 => CudaFailureStage::R4PivotUpload,
            12 => CudaFailureStage::R4PivotLaunch,
            13 => CudaFailureStage::R4PivotSynchronize,
            14 => CudaFailureStage::R4PivotCountDownload,
            15 => CudaFailureStage::R4PivotOutputDownload,
            16 => CudaFailureStage::R4PivotCapacity,
            other => CudaFailureStage::Unknown(other),
        }
    }
}

/// `(stage, error)` of the last failing CUDA call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CudaFailure {
    pub stage: CudaFailureStage,
    pub code: i32,
}

/// Loaded accelerator ABI and source build identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceleratorIdentity {
    pub abi: i32,
    pub build_id: String,
}

/// What the loaded accelerator can actually do.
///
/// `available` is `false` only for a backend that has no loaded library, so a
/// handshake can never advertise a capability it did not probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCapabilities {
    /// Whether a library passed ABI, policy and symbol resolution.
    pub available: bool,
    /// `seed_accelerator_abi_version()`.
    pub abi: i32,
    /// `seed_accelerator_build_id()`.
    pub build_id: String,
    /// A real probe of `cuda_seed_acceleration_available()`.
    pub cuda_seed_acceleration: bool,
    /// Always true: the default policy is strict GPU, so bulk CPU is opt-in.
    pub bulk_cpu_requires_opt_in: bool,
    /// `None` until the effect-preimage DirectCompute probe is scheduled.
    pub directcompute_effect_filter: Option<bool>,
}

impl NativeCapabilities {
    /// Capabilities of an absent accelerator: no probe succeeded.
    pub fn absent() -> Self {
        Self {
            available: false,
            abi: 0,
            build_id: String::new(),
            cuda_seed_acceleration: false,
            bulk_cpu_requires_opt_in: true,
            directcompute_effect_filter: None,
        }
    }
}

/// One explicit native scan window.
///
/// `trial` values in results are 1-based flat cursors over
/// `values.len() * 0x10000`, so `start_index`/`stop_index` are exclusive
/// cursor bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PivotWindow {
    pub start_index: u64,
    pub stop_index: u64,
    pub low16_stride: u16,
    pub draw_index: u32,
}

/// A native pivot match: the seed and the 1-based trial that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PivotMatch {
    pub seed: u32,
    pub trial: u64,
}

/// Verdict of one native call the Rust layer rejects or the DLL refuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeSearchError {
    /// No library was loaded, so no native call is possible.
    Unavailable,
    /// The Rust side refused the input before any DLL call.
    InvalidInput(&'static str),
    /// The DLL rejected an input the Rust side accepted.
    Rejected { call: &'static str },
    /// CUDA was unusable and the strict-GPU policy forbid the CPU path.
    CudaUnavailable {
        stage: Option<CudaFailureStage>,
        code: i32,
    },
    /// `seed_accelerator_set_execution_policy` refused the policy.
    PolicyRejected,
    /// The effect-preimage helper is unusable, carrying the pinned reason (an
    /// absent library, a substituted artifact, a missing export or a machine
    /// with no DirectCompute device). The reason is never collapsed into a
    /// generic failure: the caller has to know whether to install, replace or
    /// change policy.
    PreimageUnavailable(String),
}

/// One composed auxiliary page, including the native stage counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuxiliaryPivotPage {
    pub matches: Vec<PivotMatch>,
    /// `1 + has_terrain_constraint + enemy_group_count + rule_group_count`
    /// cumulative counts; index 0 is the natural-seed count for the window.
    pub stage_counts: Vec<u64>,
    pub backend: NativeBackend,
}

/// Inputs of `collect_ng3_r4_primary_pivot_seeds`.
///
/// The lookup blobs are the little-endian bytes the Python reference builds,
/// so `normal_lookups.len() == context_count * LOOKUP_ENTRIES * 4`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct R4PrimaryPivotSpec {
    pub allowed_effect_ids: Vec<u32>,
    pub context_by_first_u16: Vec<u8>,
    pub context_count: u32,
    pub normal_lookups: Vec<u8>,
    pub promoted_lookups: Vec<u8>,
    pub promotion_success_lookup: Vec<u8>,
    pub random7_lookup: Vec<u8>,
}

/// Inputs of `generate_ng3_primary_effect_ids_context`.
///
/// The shipped NG3 rarity-3 primary replay builds one weighted lottery per
/// special context and asks the DLL for the primary effect id of a whole batch
/// of seeds. `allowed_effect_ids` is the caller's request: a seed is accepted
/// when its generated primary id is one of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimaryEffectSpec {
    pub allowed_effect_ids: Vec<u32>,
    pub normal_lookup: Vec<u32>,
    pub promoted_lookup: Vec<u32>,
    pub promotion_success_lookup: Vec<u8>,
    pub random7_lookup: Vec<u8>,
    pub pre_promotion_draws: u32,
    pub slot_limit: u8,
    pub excluded_slot_mask: u8,
    pub primary_source_index: u8,
}

/// Inputs of `collect_auxiliary_pivot_matches`.
///
/// Criterion groups are alternatives: every group must match at least one key.
/// `enemy_criterion_groups` covers the native enemy groups followed by the
/// scratch groups, in that order, and `rule_criterion_groups` holds `u16`
/// special-rule keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuxiliaryPivotSpec {
    pub draw_index: u32,
    pub playthrough: u8,
    pub mode_threshold: i32,
    pub filtered_terrain_rows: Vec<u32>,
    pub terrain_row_count: u32,
    pub allowed_terrain_rows: Vec<u8>,
    pub has_terrain_constraint: bool,
    pub descriptor_thresholds: [i32; 3],
    pub selector_threshold: i32,
    pub role_five_threshold: i32,
    pub selector_value: u8,
    pub enemy_rows: Vec<u8>,
    pub terrains: Vec<u8>,
    pub contexts: Vec<u8>,
    pub enemy_criterion_groups: Vec<Vec<u32>>,
    pub enemy_group_count: u32,
    pub scratch_group_count: u32,
    pub rule_rows: Vec<u8>,
    pub rule_criterion_groups: Vec<Vec<u16>>,
}

impl AuxiliaryPivotSpec {
    /// The stage-count vector length the native ABI requires.
    pub fn stage_count(&self) -> usize {
        1 + usize::from(self.has_terrain_constraint)
            + self.enemy_group_count as usize
            + self.rule_criterion_groups.len()
    }
}

/// Flatten criterion groups into `(keys, offsets)`, first group at offset 0.
fn flatten_groups<T: Copy + Ord>(
    groups: &[Vec<T>],
) -> Result<(Vec<T>, Vec<u16>), NativeSearchError> {
    let mut keys: Vec<T> = Vec::new();
    let mut offsets: Vec<u16> = Vec::with_capacity(groups.len() + 1);
    offsets.push(0);
    for group in groups {
        if group.is_empty() {
            return Err(NativeSearchError::InvalidInput(
                "native criterion groups cannot be empty",
            ));
        }
        let mut sorted = group.clone();
        sorted.sort_unstable();
        keys.extend_from_slice(&sorted);
        if keys.len() > u16::MAX as usize {
            return Err(NativeSearchError::InvalidInput(
                "native criterion alternatives exceed the uint16 ABI",
            ));
        }
        offsets.push(keys.len() as u16);
    }
    Ok((keys, offsets))
}

/// `(1 << groups) - 1`: the mask a seed must set when every group matches.
///
/// The native ABI caps a batch at 32 groups, where the mask is all ones.
fn group_mask(groups: usize) -> u32 {
    if groups >= 32 {
        u32::MAX
    } else {
        (1u32 << groups) - 1
    }
}

/// The loaded accelerator library, owned by the single worker job owner.
pub struct Accelerator {
    identity: AcceleratorIdentity,
    native: platform::Native,
}

// SAFETY: the handle is a process-global module handle that stays loaded for
// the process lifetime, so calling its exports from another thread is sound.
// All execution-policy mutation goes through the process-wide policy lock and
// every native call is serialized by `search_backend`'s process-wide call lock,
// so the accelerator is safe to move to and share with the worker's job thread.
unsafe impl Send for Accelerator {}
// SAFETY: see above. The DLL keeps process-global diagnostics, so concurrent
// calls are serialized rather than raced.
unsafe impl Sync for Accelerator {}

impl Accelerator {
    /// Load and validate `bin/nioh3_seed_accelerator.dll`.
    ///
    /// `override_path` is `NIOH3_SEED_ACCELERATOR`. A missing file, a wrong ABI
    /// version, an unresolvable export or a rejected strict-GPU policy all
    /// yield `None`, exactly like `seed_accelerator._load_accelerator`.
    pub fn load(application_root: &Path, override_path: Option<&Path>) -> Option<Self> {
        let module_path = match override_path {
            Some(path) => path.to_path_buf(),
            None => application_root
                .join("bin")
                .join("nioh3_seed_accelerator.dll"),
        };
        let native = platform::Native::load(&module_path)?;
        let identity = AcceleratorIdentity {
            abi: SEED_ACCELERATOR_ABI_VERSION,
            build_id: native.build_id(),
        };
        Some(Self { identity, native })
    }

    /// The loaded ABI version and source build identity.
    pub fn identity(&self) -> AcceleratorIdentity {
        self.identity.clone()
    }

    /// Real capability probe of the loaded library.
    pub fn capabilities(&self) -> NativeCapabilities {
        NativeCapabilities {
            available: true,
            abi: self.identity.abi,
            build_id: self.identity.build_id.clone(),
            cuda_seed_acceleration: self.native.cuda_available(),
            bulk_cpu_requires_opt_in: true,
            directcompute_effect_filter: None,
        }
    }

    /// Which path served the last native call.
    pub fn last_backend(&self) -> NativeBackend {
        NativeBackend::from_raw(self.native.last_backend())
    }

    /// The last CUDA failure, or `None` when the last call did not fail.
    pub fn last_cuda_failure(&self) -> Option<CudaFailure> {
        let stage = self.native.last_cuda_stage();
        let code = self.native.last_cuda_error();
        if stage == 0 && code == 0 {
            return None;
        }
        Some(CudaFailure {
            stage: CudaFailureStage::from_raw(stage),
            code,
        })
    }

    /// Install a policy for one operation and restore on drop.
    ///
    /// Mirrors `seed_acceleration_execution_policy`: the outermost guard always
    /// restores strict GPU, so a failed or cancelled operation cannot leak the
    /// bulk-CPU opt-in into the next job.
    pub fn pin_policy(
        &self,
        policy: ExecutionPolicy,
    ) -> Result<ExecutionPolicyGuard<'_>, NativeSearchError> {
        // The native install and the lock's mirror change together under one
        // acquisition, so no concurrent load probe can read a policy the
        // library is not already holding.
        let previous = policy_lock::install(policy.raw(), |raw| self.native.set_policy(raw))
            .map_err(|()| NativeSearchError::PolicyRejected)?;
        Ok(ExecutionPolicyGuard {
            accelerator: self,
            previous,
            _not_send: std::marker::PhantomData,
        })
    }

    /// The policy currently installed for the calling thread.
    pub fn pinned_policy(&self) -> ExecutionPolicy {
        if policy_lock::active() == EXECUTION_POLICY_ALLOW_BULK_CPU {
            ExecutionPolicy::AllowBulkCpu
        } else {
            ExecutionPolicy::StrictGpu
        }
    }

    fn fail(&self, call: &'static str) -> NativeSearchError {
        if self.last_backend() == NativeBackend::CpuBlocked {
            let failure = self.last_cuda_failure();
            return NativeSearchError::CudaUnavailable {
                stage: failure.map(|value| value.stage),
                code: failure.map_or(0, |value| value.code),
            };
        }
        NativeSearchError::Rejected { call }
    }

    /// `collect_natural_pivot_seeds` over one bounded window.
    pub fn collect_natural_pivot_page(
        &self,
        values: &[u16],
        window: PivotWindow,
    ) -> Result<Vec<PivotMatch>, NativeSearchError> {
        if values.is_empty() || window.start_index > window.stop_index {
            return Err(NativeSearchError::InvalidInput(
                "invalid native pivot range",
            ));
        }
        if !(1..=64).contains(&window.draw_index) {
            return Err(NativeSearchError::InvalidInput(
                "native pivot draw index must be in 1..64",
            ));
        }
        let capacity = window.stop_index - window.start_index;
        if capacity == 0 {
            return Ok(Vec::new());
        }
        if capacity > MAX_NATURAL_TRIALS {
            return Err(NativeSearchError::InvalidInput(
                "native pivot calls must not exceed 1,000,000 trials",
            ));
        }
        let mut seeds = vec![0u32; capacity as usize];
        let mut trials = vec![0u64; capacity as usize];
        let count = self.native.collect_natural_pivot_seeds(
            values,
            window,
            &mut seeds,
            &mut trials,
            capacity,
        );
        if count == ERROR_RESULT || count > capacity {
            return Err(self.fail("collect_natural_pivot_seeds"));
        }
        Ok(sorted_matches(&seeds, &trials, count))
    }

    /// `collect_ng3_r4_primary_pivot_seeds` over one bounded window.
    pub fn collect_r4_primary_pivot_page(
        &self,
        values: &[u16],
        spec: &R4PrimaryPivotSpec,
        window: PivotWindow,
    ) -> Result<Vec<PivotMatch>, NativeSearchError> {
        if values.is_empty() {
            return Err(NativeSearchError::InvalidInput(
                "invalid native R4 primary pivot range",
            ));
        }
        if spec.allowed_effect_ids.is_empty() {
            return Err(NativeSearchError::InvalidInput(
                "at least one R4 primary effect must be selected",
            ));
        }
        if window.low16_stride == 0 || window.low16_stride.is_multiple_of(2) {
            return Err(NativeSearchError::InvalidInput(
                "low16_stride must be an odd uint16",
            ));
        }
        if spec.context_by_first_u16.len() != LOOKUP_ENTRIES
            || !(1..=0x100).contains(&spec.context_count)
        {
            return Err(NativeSearchError::InvalidInput(
                "invalid R4 primary context lookup",
            ));
        }
        let matrix_size = spec.context_count as usize * LOOKUP_ENTRIES * 4;
        if spec.normal_lookups.len() != matrix_size || spec.promoted_lookups.len() != matrix_size {
            return Err(NativeSearchError::InvalidInput(
                "invalid R4 primary lookup matrices",
            ));
        }
        if spec.promotion_success_lookup.len() != LOOKUP_ENTRIES
            || spec.random7_lookup.len() != LOOKUP_ENTRIES
        {
            return Err(NativeSearchError::InvalidInput("invalid R4 path lookups"));
        }
        let trial_count = window.stop_index.checked_sub(window.start_index).ok_or(
            NativeSearchError::InvalidInput("invalid native R4 primary pivot range"),
        )?;
        if trial_count > MAX_R4_PRIMARY_TRIALS {
            return Err(NativeSearchError::InvalidInput(
                "invalid native R4 primary pivot range",
            ));
        }
        if trial_count == 0 {
            return Ok(Vec::new());
        }
        let capacity = trial_count.min(MAX_R4_PRIMARY_CAPACITY);
        let mut seeds = vec![0u32; capacity as usize];
        let mut trials = vec![0u64; capacity as usize];
        let count = self.native.collect_r4_primary_pivot_seeds(
            values,
            spec,
            window,
            &mut seeds,
            &mut trials,
            capacity,
        );
        if count == ERROR_RESULT || count > capacity {
            return Err(self.fail("collect_ng3_r4_primary_pivot_seeds"));
        }
        Ok(sorted_matches(&seeds, &trials, count))
    }

    /// `collect_auxiliary_pivot_matches` over one bounded window.
    pub fn collect_auxiliary_pivot_page(
        &self,
        values: &[u16],
        spec: &AuxiliaryPivotSpec,
        window: PivotWindow,
        output_capacity: u64,
    ) -> Result<AuxiliaryPivotPage, NativeSearchError> {
        validate_auxiliary(values, spec, window, output_capacity)?;
        let item_count = window.stop_index - window.start_index;
        let capacity = output_capacity.min(item_count.max(1));
        let stage_count = spec.stage_count();
        let mut seeds = vec![0u32; capacity as usize];
        let mut trials = vec![0u64; capacity as usize];
        let mut stage_counts = vec![0u64; stage_count];
        let count = self.native.collect_auxiliary_pivot_matches(
            values,
            spec,
            window,
            &mut seeds,
            &mut trials,
            capacity,
            &mut stage_counts,
        );
        if count == ERROR_RESULT || count > capacity {
            return Err(self.fail("collect_auxiliary_pivot_matches"));
        }
        Ok(AuxiliaryPivotPage {
            matches: sorted_matches(&seeds, &trials, count),
            stage_counts,
            backend: self.last_backend(),
        })
    }

    /// The shipped batched primary-effect predicate, in one verdict per seed.
    ///
    /// Mirrors `effect_sequence.generate_ng3_rarity34_primary_effect_ids` for
    /// rarity 3: the DLL returns the primary effect id every seed would roll,
    /// and a seed is selected when that id is one the caller asked for. This is
    /// the predicate the shipped solver applies to the full seed family before
    /// it replays an effect sequence or composes a candidate.
    pub fn primary_effect_selected(
        &self,
        spec: &PrimaryEffectSpec,
        seeds: &[u32],
    ) -> Result<Vec<bool>, NativeSearchError> {
        if seeds.is_empty() {
            return Ok(Vec::new());
        }
        if spec.allowed_effect_ids.is_empty() {
            return Ok(vec![true; seeds.len()]);
        }
        if spec.normal_lookup.len() != LOOKUP_ENTRIES
            || spec.promoted_lookup.len() != LOOKUP_ENTRIES
            || spec.promotion_success_lookup.len() != LOOKUP_ENTRIES
            || spec.random7_lookup.len() != LOOKUP_ENTRIES
            || spec.slot_limit == 0
            || spec.slot_limit > 7
            || spec.primary_source_index >= spec.slot_limit
            || spec.excluded_slot_mask & (1u8 << spec.primary_source_index) != 0
        {
            return Err(NativeSearchError::InvalidInput(
                "native primary predicate inputs do not match the API",
            ));
        }
        let mut selected = vec![false; seeds.len()];
        for start in (0..seeds.len()).step_by(MAX_PREDICATE_BATCH) {
            let stop = (start + MAX_PREDICATE_BATCH).min(seeds.len());
            let batch = &seeds[start..stop];
            let mut generated = vec![0u32; batch.len()];
            let code = self.native.primary_effect_ids(batch, spec, &mut generated);
            if code != 0 && code != 1 {
                return Err(self.fail("generate_ng3_primary_effect_ids_context"));
            }
            for (index, effect_id) in generated.iter().enumerate() {
                selected[start + index] = spec.allowed_effect_ids.contains(effect_id);
            }
        }
        Ok(selected)
    }

    /// The shipped per-seed auxiliary predicate, in one verdict per seed.
    ///
    /// This is the placement `effect_seed_solver._iter_solution_prefetch` uses
    /// before the R4-primary route composes anything: the terrain row for every
    /// seed, the caller's terrain criteria over that row, the requested enemy
    /// groups, and the special-rule groups after their scratch-key replay. All
    /// four steps are the same native exports the fused auxiliary route uses, so
    /// no criterion semantics are re-derived in Rust.
    ///
    /// Later stages only run for seeds an earlier stage kept, mirroring the
    /// reference's staged batch filtering.
    pub fn auxiliary_criteria_selected(
        &self,
        spec: &AuxiliaryPivotSpec,
        seeds: &[u32],
    ) -> Result<Vec<bool>, NativeSearchError> {
        if seeds.is_empty() {
            return Ok(Vec::new());
        }
        if seeds.len() > MAX_PREDICATE_BATCH {
            return Err(NativeSearchError::InvalidInput(
                "native predicate batches are limited to 1,000,000 seeds",
            ));
        }
        let enemy_groups = spec.enemy_group_count as usize;
        if enemy_groups > spec.enemy_criterion_groups.len() {
            return Err(NativeSearchError::InvalidInput(
                "native enemy group count exceeds the packed groups",
            ));
        }
        let terrain_groups = &spec.enemy_criterion_groups[..enemy_groups];
        let scratch_groups = &spec.enemy_criterion_groups[enemy_groups..];
        let needs_rows = spec.has_terrain_constraint
            || !terrain_groups.is_empty()
            || !spec.rule_criterion_groups.is_empty();
        if !needs_rows {
            return Ok(vec![true; seeds.len()]);
        }
        if spec.terrain_row_count == 0
            || spec.filtered_terrain_rows.is_empty()
            || spec.allowed_terrain_rows.len() != spec.terrain_row_count as usize
        {
            return Err(NativeSearchError::InvalidInput(
                "native terrain predicate needs one flag per terrain row",
            ));
        }
        if spec
            .filtered_terrain_rows
            .iter()
            .any(|row| *row >= spec.terrain_row_count)
            || !spec.enemy_rows.len().is_multiple_of(ENEMY_ROW_BYTES)
            || !spec.terrains.len().is_multiple_of(TERRAIN_ROW_BYTES)
            || !spec.contexts.len().is_multiple_of(CONTEXT_ROW_BYTES)
            || (!terrain_groups.is_empty() || !scratch_groups.is_empty())
                && (spec.enemy_rows.is_empty()
                    || spec.terrains.is_empty()
                    || spec.contexts.is_empty())
        {
            return Err(NativeSearchError::InvalidInput(
                "native predicate inputs do not match the packed ABI",
            ));
        }
        if !spec.rule_criterion_groups.is_empty()
            && (!spec.rule_rows.len().is_multiple_of(RULE_ROW_BYTES) || spec.rule_rows.is_empty())
        {
            return Err(NativeSearchError::InvalidInput(
                "native special-rule rows do not match the 16-byte ABI",
            ));
        }

        let mut selected = vec![true; seeds.len()];
        for start in (0..seeds.len()).step_by(MAX_PREDICATE_BATCH) {
            let stop = (start + MAX_PREDICATE_BATCH).min(seeds.len());
            let batch = &seeds[start..stop];
            let mut rows = vec![0u32; batch.len()];
            let code = self.native.terrain_row_indices(
                batch,
                spec.mode_threshold,
                &spec.filtered_terrain_rows,
                spec.terrain_row_count,
                &mut rows,
            );
            if code != 0 && code != 1 {
                return Err(self.fail("generate_terrain_row_indices"));
            }
            if spec.has_terrain_constraint {
                for (index, row) in rows.iter().enumerate() {
                    if spec.allowed_terrain_rows[*row as usize] == 0 {
                        selected[start + index] = false;
                    }
                }
            }
            if !terrain_groups.is_empty() {
                let kept: Vec<usize> = (0..batch.len())
                    .filter(|index| selected[start + index])
                    .collect();
                let masks = self.enemy_masks(&kept, batch, &rows, spec, terrain_groups)?;
                let target = group_mask(terrain_groups.len());
                for (slot, mask) in masks.iter().enumerate() {
                    if *mask != target {
                        selected[start + kept[slot]] = false;
                    }
                }
            }
            if !spec.rule_criterion_groups.is_empty() {
                let kept: Vec<usize> = (0..batch.len())
                    .filter(|index| selected[start + index])
                    .collect();
                let scratch = self.enemy_masks(&kept, batch, &rows, spec, scratch_groups)?;
                let batch_seeds: Vec<u32> = kept.iter().map(|index| batch[*index]).collect();
                let mut scratch_for_rules = vec![0u32; scratch.len()];
                scratch_for_rules.copy_from_slice(&scratch);
                let masks = self.rule_masks(&batch_seeds, &scratch_for_rules, spec)?;
                let target = group_mask(spec.rule_criterion_groups.len());
                for (slot, mask) in masks.iter().enumerate() {
                    if *mask != target {
                        selected[start + kept[slot]] = false;
                    }
                }
            }
        }
        Ok(selected)
    }

    /// Enemy masks for the kept seeds of one batch, in kept order.
    fn enemy_masks(
        &self,
        kept: &[usize],
        batch: &[u32],
        rows: &[u32],
        spec: &AuxiliaryPivotSpec,
        groups: &[Vec<u32>],
    ) -> Result<Vec<u32>, NativeSearchError> {
        if kept.is_empty() {
            return Ok(Vec::new());
        }
        if groups.is_empty() || groups.len() > 32 || groups.iter().any(|group| group.is_empty()) {
            return Err(NativeSearchError::InvalidInput(
                "native enemy matching requires 1..32 non-empty groups",
            ));
        }
        let seeds: Vec<u32> = kept.iter().map(|index| batch[*index]).collect();
        let terrain_rows: Vec<u32> = kept.iter().map(|index| rows[*index]).collect();
        let mut masks = vec![0u32; seeds.len()];
        let code = self.native.match_enemy_constraints(
            &seeds,
            &terrain_rows,
            spec.playthrough,
            spec.mode_threshold,
            &spec.descriptor_thresholds,
            spec.selector_threshold,
            spec.role_five_threshold,
            spec.selector_value,
            &spec.enemy_rows,
            &spec.terrains,
            &spec.contexts,
            groups,
            &mut masks,
        );
        if code != 0 && code != 1 {
            return Err(self.fail("match_enemy_constraints"));
        }
        Ok(masks)
    }

    /// Special-rule masks for one batch after the scratch-key replay.
    fn rule_masks(
        &self,
        seeds: &[u32],
        scratch_masks: &[u32],
        spec: &AuxiliaryPivotSpec,
    ) -> Result<Vec<u32>, NativeSearchError> {
        if seeds.is_empty() {
            return Ok(Vec::new());
        }
        if spec.rule_criterion_groups.is_empty()
            || spec.rule_criterion_groups.len() > 32
            || spec
                .rule_criterion_groups
                .iter()
                .any(|group| group.is_empty())
        {
            return Err(NativeSearchError::InvalidInput(
                "native special-rule matching requires 1..32 non-empty groups",
            ));
        }
        let mut masks = vec![0u32; seeds.len()];
        let code = self.native.match_special_rule_constraints(
            seeds,
            scratch_masks,
            &spec.rule_rows,
            &spec.rule_criterion_groups,
            &mut masks,
        );
        if code != 0 && code != 1 {
            return Err(self.fail("match_special_rule_constraints"));
        }
        Ok(masks)
    }

    /// Test-only hook: `seed_accelerator_test_force_cuda_failure`.
    ///
    /// The shipped DLL exports this; it exists so the strict-GPU refusal and the
    /// bulk-CPU opt-in can be verified on a CUDA-capable machine.
    #[doc(hidden)]
    pub fn force_cuda_failure(&self, enabled: bool) {
        self.native.force_cuda_failure(enabled);
    }

    /// Test-only hook: `seed_accelerator_bulk_cpu_call_count`.
    #[doc(hidden)]
    pub fn bulk_cpu_call_count(&self) -> u64 {
        self.native.bulk_cpu_call_count()
    }

    /// Build the exact 65,536-entry inclusive weighted lottery table.
    ///
    /// Mirrors `build_weighted_effect_lookup_native`: the shipped DLL is
    /// authoritative so the R4 primary lookups match the CUDA reference bit for
    /// bit instead of being recomputed with a second rounding rule.
    pub fn build_weighted_effect_lookup(
        &self,
        entries: &[(u32, u32)],
    ) -> Result<Vec<u32>, NativeSearchError> {
        if entries.is_empty() || entries.len() > 4096 {
            return Err(NativeSearchError::InvalidInput(
                "weighted lookup requires 1..4,096 entries",
            ));
        }
        let effect_ids: Vec<u32> = entries.iter().map(|entry| entry.0).collect();
        let weights: Vec<u32> = entries.iter().map(|entry| entry.1).collect();
        let mut output = vec![0u32; LOOKUP_ENTRIES];
        let result = self
            .native
            .build_weighted_lookup(&effect_ids, &weights, &mut output);
        if result != 0 {
            return Err(NativeSearchError::Rejected {
                call: "build_weighted_effect_lookup",
            });
        }
        Ok(output)
    }
}

/// Sort native output by its canonical mathematical cursor.
///
/// CUDA threads append atomically and therefore have no stable order; sorting
/// by trial keeps pagination exact and makes GPU and CPU byte-identical.
fn sorted_matches(seeds: &[u32], trials: &[u64], count: u64) -> Vec<PivotMatch> {
    let mut matches: Vec<PivotMatch> = (0..count as usize)
        .map(|index| PivotMatch {
            seed: seeds[index],
            trial: trials[index],
        })
        .collect();
    matches.sort_by_key(|item| item.trial);
    matches
}

fn validate_auxiliary(
    values: &[u16],
    spec: &AuxiliaryPivotSpec,
    window: PivotWindow,
    output_capacity: u64,
) -> Result<(), NativeSearchError> {
    let invalid = NativeSearchError::InvalidInput;
    if values.is_empty() || window.start_index > window.stop_index {
        return Err(invalid("invalid native auxiliary pivot range"));
    }
    if window.stop_index - window.start_index > MAX_AUXILIARY_TRIALS {
        return Err(invalid(
            "native auxiliary pivot calls must not exceed 8,000,000 trials",
        ));
    }
    if !(1..=64).contains(&window.draw_index) {
        return Err(invalid("native pivot draw index must be in 1..64"));
    }
    if !(1..=5).contains(&spec.playthrough) {
        return Err(invalid("playthrough must be in 1..5"));
    }
    if spec.filtered_terrain_rows.is_empty() || spec.terrain_row_count == 0 {
        return Err(invalid("native terrain configuration cannot be empty"));
    }
    if spec.allowed_terrain_rows.len() != spec.terrain_row_count as usize {
        return Err(invalid(
            "terrain allow-mask must contain one byte per native row",
        ));
    }
    if spec.enemy_rows.is_empty() || !spec.enemy_rows.len().is_multiple_of(ENEMY_ROW_BYTES) {
        return Err(invalid("packed enemy rows must use the 18-byte native ABI"));
    }
    if !spec.terrains.len().is_multiple_of(TERRAIN_ROW_BYTES)
        || spec.terrains.len() / TERRAIN_ROW_BYTES != spec.terrain_row_count as usize
    {
        return Err(invalid(
            "packed terrain rows must use the 5-byte native ABI",
        ));
    }
    if spec.contexts.is_empty() || !spec.contexts.len().is_multiple_of(CONTEXT_ROW_BYTES) {
        return Err(invalid("packed contexts must use the 22-byte native ABI"));
    }
    if spec.enemy_group_count as usize + spec.scratch_group_count as usize
        != spec.enemy_criterion_groups.len()
    {
        return Err(invalid(
            "native enemy and scratch group counts do not match",
        ));
    }
    if spec.enemy_criterion_groups.len() > MAX_CRITERION_GROUPS
        || spec.rule_criterion_groups.len() > MAX_CRITERION_GROUPS
    {
        return Err(invalid(
            "native auxiliary matching supports at most 32 groups",
        ));
    }
    if spec.rule_criterion_groups.is_empty() != spec.rule_rows.is_empty() {
        return Err(invalid(
            "native rule rows and criterion groups must be supplied together",
        ));
    }
    if !spec.rule_rows.is_empty() && !spec.rule_rows.len().is_multiple_of(RULE_ROW_BYTES) {
        return Err(invalid(
            "packed special-rule rows must use the 16-byte native ABI",
        ));
    }
    if spec.enemy_rows.len() / ENEMY_ROW_BYTES > MAX_ENEMY_ROWS {
        return Err(invalid("packed enemy rows exceed the native row limit"));
    }
    if spec.rule_rows.len() / RULE_ROW_BYTES > MAX_RULE_ROWS {
        return Err(invalid(
            "packed special-rule rows exceed the native row limit",
        ));
    }
    if output_capacity == 0 {
        return Err(invalid("native auxiliary output capacity must be positive"));
    }
    flatten_groups(&spec.enemy_criterion_groups)?;
    flatten_groups(&spec.rule_criterion_groups)?;
    Ok(())
}

/// RAII execution-policy guard; drop restores the policy that was active when
/// it was created, and the outermost drop restores strict GPU.
pub struct ExecutionPolicyGuard<'a> {
    accelerator: &'a Accelerator,
    previous: i32,
    _not_send: std::marker::PhantomData<Rc<()>>,
}

impl Drop for ExecutionPolicyGuard<'_> {
    fn drop(&mut self) {
        // Restore the native policy and the lock's mirror under one
        // acquisition: a probe racing this drop must not read the policy this
        // guard installed and write it back after the restore.
        policy_lock::restore(self.previous, |raw| self.accelerator.native.set_policy(raw));
    }
}

/// Re-entrant, thread-aware policy lock mirroring Python's `RLock`.
///
/// The lock owns the authoritative execution policy for the process, and every
/// mutation of the native policy runs inside one acquisition: a guard
/// installing an opt-in, a guard restoring it on drop, and a load or identity
/// probe re-installing the policy it finds. Holding the lock across the native
/// call is what keeps the lock's mirror and the loaded library in agreement
/// when a probe runs during an operation.
mod policy_lock {
    use std::sync::{Condvar, Mutex};
    use std::thread::{self, ThreadId};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Slot {
        owner: Option<ThreadId>,
        depth: u32,
        active: i32,
    }

    static SLOT: Mutex<Option<Slot>> = Mutex::new(None);
    static WAKE: Condvar = Condvar::new();

    fn lock_slot() -> std::sync::MutexGuard<'static, Option<Slot>> {
        SLOT.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// Install `policy` and return the policy that was active before.
    ///
    /// `set` is the native `seed_accelerator_set_execution_policy` call. It runs
    /// under the lock, so a probe re-installing the active policy and a guard
    /// changing it are serialized rather than interleaved. A policy the library
    /// rejects leaves the lock untouched.
    pub(super) fn install<F>(policy: i32, set: F) -> Result<i32, ()>
    where
        F: FnOnce(i32) -> i32,
    {
        let me = thread::current().id();
        let mut guard = lock_slot();
        loop {
            match *guard {
                None => {
                    if set(policy) != 0 {
                        return Err(());
                    }
                    *guard = Some(Slot {
                        owner: Some(me),
                        depth: 1,
                        active: policy,
                    });
                    return Ok(super::EXECUTION_POLICY_STRICT_GPU);
                }
                Some(slot) if slot.owner == Some(me) => {
                    let previous = slot.active;
                    if set(policy) != 0 {
                        return Err(());
                    }
                    if let Some(slot) = guard.as_mut() {
                        slot.depth += 1;
                        slot.active = policy;
                    }
                    return Ok(previous);
                }
                Some(_) => {
                    guard = WAKE.wait(guard).unwrap_or_else(|error| error.into_inner());
                }
            }
        }
    }

    /// Restore `previous` for one guard drop and release one acquisition.
    ///
    /// The native restore runs under the lock too, so the mirror never
    /// advertises a policy the loaded library has not been given.
    pub(super) fn restore<F>(previous: i32, set: F)
    where
        F: FnOnce(i32) -> i32,
    {
        let wake = {
            let mut guard = lock_slot();
            let _ = set(previous);
            match guard.as_mut() {
                Some(slot) => {
                    slot.active = previous;
                    slot.depth = slot.depth.saturating_sub(1);
                    if slot.depth == 0 {
                        *guard = None;
                        true
                    } else {
                        false
                    }
                }
                None => false,
            }
        };
        if wake {
            WAKE.notify_one();
        }
    }

    /// Re-install the active policy for a load or identity probe.
    ///
    /// Reading the policy and writing it back is one acquisition: a guard
    /// installing or restoring the native policy in between would otherwise be
    /// cancelled (a fresh opt-in) or leaked (a released one).
    #[cfg(windows)]
    pub(super) fn with_active<F>(set: F) -> i32
    where
        F: FnOnce(i32) -> i32,
    {
        let guard = lock_slot();
        let active = guard.map_or(super::EXECUTION_POLICY_STRICT_GPU, |slot| slot.active);
        set(active)
    }

    /// The policy currently installed for the lock owner.
    pub(super) fn active() -> i32 {
        lock_slot().map_or(super::EXECUTION_POLICY_STRICT_GPU, |slot| slot.active)
    }
}

impl Default for NativeCapabilities {
    fn default() -> Self {
        Self::absent()
    }
}

/// Capabilities of an absent accelerator.
///
/// The handshake path uses this when `SearchBackend::load` returned `None`, so
/// the worker can always answer `handshake` and still refuse search.
pub fn absent_capabilities() -> NativeCapabilities {
    NativeCapabilities::absent()
}

/// Re-install the policy the process-global policy lock currently holds.
///
/// A load or identity probe must not hard-code strict GPU: an operation-scoped
/// bulk-CPU guard may already be installed, and cancelling that opt-in would
/// fail the running operation. `set_policy` is the probe's resolved native
/// `seed_accelerator_set_execution_policy`; the read and the write run inside
/// one lock acquisition, so a guard installing or restoring the native policy
/// between them can neither be cancelled nor leaked. Strict GPU is the value
/// installed while no guard is active.
#[cfg(windows)]
pub(crate) fn reinstate_active_policy<F>(set_policy: F) -> i32
where
    F: FnOnce(i32) -> i32,
{
    policy_lock::with_active(set_policy)
}

/// The platform ABI. All raw FFI lives here.
#[cfg(windows)]
mod platform {
    use std::ffi::{c_void, CStr};
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use super::{
        flatten_groups, AuxiliaryPivotSpec, PivotWindow, PrimaryEffectSpec, R4PrimaryPivotSpec,
        LOOKUP_ENTRIES, SEED_ACCELERATOR_ABI_VERSION,
    };

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }

    type AbiVersion = unsafe extern "C" fn() -> i32;
    type BuildId = unsafe extern "C" fn() -> *const i8;
    type SetExecutionPolicy = unsafe extern "C" fn(i32) -> i32;
    type CudaAvailable = unsafe extern "C" fn() -> i32;
    type LastBackend = unsafe extern "C" fn() -> i32;
    type LastCudaError = unsafe extern "C" fn() -> i32;
    type ForceCudaFailure = unsafe extern "C" fn(i32);
    type BulkCpuCallCount = unsafe extern "C" fn() -> u64;
    type BuildWeightedLookup = unsafe extern "C" fn(*const u32, *const u32, u32, *mut u32) -> i32;
    type CollectNatural =
        unsafe extern "C" fn(*const u16, u32, u64, u64, u16, u32, *mut u32, *mut u64, u64) -> u64;
    type CollectR4Primary = unsafe extern "C" fn(
        *const u16,
        u32,
        u64,
        u64,
        u16,
        *const u32,
        u32,
        *const u8,
        u32,
        *const u32,
        *const u32,
        *const u8,
        *const u8,
        *mut u32,
        *mut u64,
        u64,
    ) -> u64;
    #[allow(clippy::type_complexity)]
    type CollectAuxiliary = unsafe extern "C" fn(
        *const u16,
        u32,
        u64,
        u64,
        u16,
        u32,
        u8,
        i32,
        *const u32,
        u32,
        u32,
        *const u8,
        u32,
        *const i32,
        i32,
        i32,
        u8,
        *const c_void,
        u32,
        *const c_void,
        u32,
        *const c_void,
        u32,
        *const u32,
        u32,
        *const u16,
        u32,
        u32,
        *const c_void,
        u32,
        *const u16,
        u32,
        *const u16,
        u32,
        *mut u32,
        *mut u64,
        u64,
        *mut u64,
        u32,
    ) -> u64;

    type TerrainRows =
        unsafe extern "C" fn(*const u32, u64, i32, *const u32, u32, u32, *mut u32) -> i32;
    #[allow(clippy::type_complexity)]
    type MatchEnemy = unsafe extern "C" fn(
        *const u32,
        *const u32,
        u64,
        u8,
        i32,
        *const i32,
        i32,
        i32,
        u8,
        *const c_void,
        u32,
        *const c_void,
        u32,
        *const c_void,
        u32,
        *const u32,
        u32,
        *const u16,
        u32,
        *mut u32,
    ) -> i32;
    #[allow(clippy::type_complexity)]
    type MatchRules = unsafe extern "C" fn(
        *const u32,
        *const u32,
        u64,
        *const c_void,
        u32,
        *const u16,
        u32,
        *const u16,
        u32,
        *mut u32,
    ) -> i32;
    #[allow(clippy::type_complexity)]
    type PrimaryIds = unsafe extern "C" fn(
        *const u32,
        u64,
        *const u32,
        *const u32,
        *const u8,
        *const u8,
        u32,
        u8,
        u8,
        u8,
        *mut u32,
    ) -> i32;

    /// Resolved exports of one loaded accelerator library.
    pub(super) struct Native {
        /// Lives for the process lifetime; the library is never unloaded.
        _handle: *mut c_void,
        build_id: String,
        set_policy: SetExecutionPolicy,
        cuda_available: CudaAvailable,
        last_backend: LastBackend,
        last_cuda_error: LastCudaError,
        last_cuda_stage: LastCudaError,
        collect_natural: CollectNatural,
        collect_r4_primary: CollectR4Primary,
        collect_auxiliary: CollectAuxiliary,
        terrain_rows: TerrainRows,
        match_enemy: MatchEnemy,
        match_rules: MatchRules,
        primary_ids: PrimaryIds,
        build_weighted_lookup: BuildWeightedLookup,
        force_cuda_failure: Option<ForceCudaFailure>,
        bulk_cpu_call_count: Option<BulkCpuCallCount>,
    }

    // SAFETY: the module handle is process-global and never unloaded.
    unsafe impl Send for Native {}
    // SAFETY: the module handle is process-global and never unloaded.
    unsafe impl Sync for Native {}

    impl Native {
        pub(super) fn load(module_path: &Path) -> Option<Self> {
            if !module_path.is_file() {
                return None;
            }
            let mut wide: Vec<u16> = module_path.as_os_str().encode_wide().collect();
            wide.push(0);
            // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the
            // call. The handle is intentionally never freed, matching Python's
            // cached `ctypes.WinDLL`, so no call can race an unload.
            let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
            if handle.is_null() {
                return None;
            }
            // SAFETY: every symbol below is looked up by its exact exported
            // name and transmuted to the matching typed signature.
            unsafe {
                let abi_version: AbiVersion = symbol(handle, b"seed_accelerator_abi_version\0")?;
                let build_id: BuildId = symbol(handle, b"seed_accelerator_build_id\0")?;
                let set_policy: SetExecutionPolicy =
                    symbol(handle, b"seed_accelerator_set_execution_policy\0")?;
                let cuda_available: CudaAvailable =
                    symbol(handle, b"cuda_seed_acceleration_available\0")?;
                let last_backend: LastBackend = symbol(handle, b"seed_accelerator_last_backend\0")?;
                let last_cuda_error: LastCudaError =
                    symbol(handle, b"seed_accelerator_last_cuda_error\0")?;
                let last_cuda_stage: LastCudaError =
                    symbol(handle, b"seed_accelerator_last_cuda_stage\0")?;
                let collect_natural: CollectNatural =
                    symbol(handle, b"collect_natural_pivot_seeds\0")?;
                let collect_r4_primary: CollectR4Primary =
                    symbol(handle, b"collect_ng3_r4_primary_pivot_seeds\0")?;
                let collect_auxiliary: CollectAuxiliary =
                    symbol(handle, b"collect_auxiliary_pivot_matches\0")?;
                let terrain_rows: TerrainRows = symbol(handle, b"generate_terrain_row_indices\0")?;
                let match_enemy: MatchEnemy = symbol(handle, b"match_enemy_constraints\0")?;
                let match_rules: MatchRules = symbol(handle, b"match_special_rule_constraints\0")?;
                let primary_ids: PrimaryIds =
                    symbol(handle, b"generate_ng3_primary_effect_ids_context\0")?;
                let build_weighted_lookup: BuildWeightedLookup =
                    symbol(handle, b"build_weighted_effect_lookup\0")?;
                let force_cuda_failure: Option<ForceCudaFailure> =
                    symbol(handle, b"seed_accelerator_test_force_cuda_failure\0");
                let bulk_cpu_call_count: Option<BulkCpuCallCount> =
                    symbol(handle, b"seed_accelerator_bulk_cpu_call_count\0");

                if abi_version() != SEED_ACCELERATOR_ABI_VERSION {
                    return None;
                }
                // The product always starts from strict GPU; only an explicit
                // operation-scoped guard may opt into the bulk CPU path. A load
                // must not cancel an opt-in a job already installed, so it
                // re-installs the policy the lock's owner holds, which is
                // strict GPU while no guard is active, under one acquisition.
                if super::reinstate_active_policy(|policy| set_policy(policy)) != 0 {
                    return None;
                }
                let raw = build_id();
                let build_id = if raw.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(raw).to_string_lossy().into_owned()
                };
                Some(Native {
                    _handle: handle,
                    set_policy,
                    cuda_available,
                    last_backend,
                    last_cuda_error,
                    last_cuda_stage,
                    collect_natural,
                    collect_r4_primary,
                    collect_auxiliary,
                    terrain_rows,
                    match_enemy,
                    match_rules,
                    primary_ids,
                    build_weighted_lookup,
                    force_cuda_failure,
                    bulk_cpu_call_count,
                    build_id,
                })
            }
        }

        pub(super) fn build_id(&self) -> String {
            self.build_id.clone()
        }

        pub(super) fn set_policy(&self, policy: i32) -> i32 {
            // SAFETY: `set_policy` is a typed pointer to the exported setter.
            unsafe { (self.set_policy)(policy) }
        }

        pub(super) fn cuda_available(&self) -> bool {
            // SAFETY: typed pointer to the exported probe.
            unsafe { (self.cuda_available)() != 0 }
        }

        pub(super) fn last_backend(&self) -> i32 {
            // SAFETY: typed pointer to the exported accessor.
            unsafe { (self.last_backend)() }
        }

        pub(super) fn last_cuda_error(&self) -> i32 {
            // SAFETY: typed pointer to the exported accessor.
            unsafe { (self.last_cuda_error)() }
        }

        pub(super) fn last_cuda_stage(&self) -> i32 {
            // SAFETY: typed pointer to the exported accessor.
            unsafe { (self.last_cuda_stage)() }
        }

        pub(super) fn force_cuda_failure(&self, enabled: bool) {
            if let Some(function) = self.force_cuda_failure {
                // SAFETY: typed pointer to the exported test hook.
                unsafe { function(i32::from(enabled)) }
            }
        }

        pub(super) fn bulk_cpu_call_count(&self) -> u64 {
            self.bulk_cpu_call_count
                // SAFETY: typed pointer to the exported counter.
                .map_or(0, |function| unsafe { function() })
        }

        pub(super) fn build_weighted_lookup(
            &self,
            effect_ids: &[u32],
            weights: &[u32],
            output: &mut [u32],
        ) -> i32 {
            debug_assert_eq!(effect_ids.len(), weights.len());
            debug_assert_eq!(output.len(), LOOKUP_ENTRIES);
            // SAFETY: all three slices stay alive for the call; the ABI caps the
            // entry count at 4,096 and the output at 65,536 entries.
            unsafe {
                (self.build_weighted_lookup)(
                    effect_ids.as_ptr(),
                    weights.as_ptr(),
                    effect_ids.len() as u32,
                    output.as_mut_ptr(),
                )
            }
        }

        pub(super) fn collect_natural_pivot_seeds(
            &self,
            values: &[u16],
            window: PivotWindow,
            seeds: &mut [u32],
            trials: &mut [u64],
            capacity: u64,
        ) -> u64 {
            debug_assert!(values.len() <= u32::MAX as usize);
            // SAFETY: the slices outlive the call, capacity matches the
            // allocated result buffers, and the DLL only writes `capacity`
            // entries.
            unsafe {
                (self.collect_natural)(
                    values.as_ptr(),
                    values.len() as u32,
                    window.start_index,
                    window.stop_index,
                    window.low16_stride,
                    window.draw_index,
                    seeds.as_mut_ptr(),
                    trials.as_mut_ptr(),
                    capacity,
                )
            }
        }

        pub(super) fn collect_r4_primary_pivot_seeds(
            &self,
            values: &[u16],
            spec: &R4PrimaryPivotSpec,
            window: PivotWindow,
            seeds: &mut [u32],
            trials: &mut [u64],
            capacity: u64,
        ) -> u64 {
            let normal = little_endian_u32(&spec.normal_lookups);
            let promoted = little_endian_u32(&spec.promoted_lookups);
            debug_assert_eq!(spec.context_by_first_u16.len(), LOOKUP_ENTRIES);
            // SAFETY: every pointer refers to a live slice whose length was
            // validated against the ABI before marshalling.
            unsafe {
                (self.collect_r4_primary)(
                    values.as_ptr(),
                    values.len() as u32,
                    window.start_index,
                    window.stop_index,
                    window.low16_stride,
                    spec.allowed_effect_ids.as_ptr(),
                    spec.allowed_effect_ids.len() as u32,
                    spec.context_by_first_u16.as_ptr(),
                    spec.context_count,
                    normal.as_ptr(),
                    promoted.as_ptr(),
                    spec.promotion_success_lookup.as_ptr(),
                    spec.random7_lookup.as_ptr(),
                    seeds.as_mut_ptr(),
                    trials.as_mut_ptr(),
                    capacity,
                )
            }
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn collect_auxiliary_pivot_matches(
            &self,
            values: &[u16],
            spec: &AuxiliaryPivotSpec,
            window: PivotWindow,
            seeds: &mut [u32],
            trials: &mut [u64],
            capacity: u64,
            stage_counts: &mut [u64],
        ) -> u64 {
            let (enemy_keys, enemy_offsets) =
                flatten_groups(&spec.enemy_criterion_groups).unwrap_or_default();
            let (rule_keys, rule_offsets) =
                flatten_groups(&spec.rule_criterion_groups).unwrap_or_default();
            let (enemy_keys_ptr, enemy_key_count) = raw_u32(&enemy_keys);
            let (enemy_offsets_ptr, _) = raw_u16(&enemy_offsets);
            let (rule_keys_ptr, rule_key_count) = raw_u16(&rule_keys);
            let (rule_offsets_ptr, _) = raw_u16(&rule_offsets);
            let rule_rows = spec.rule_rows.as_ptr();
            // SAFETY: lengths and widths were validated against the native ABI
            // before this call; optional row/rule pointers stay null when the
            // corresponding criterion group list is empty, which is the
            // documented ABI for "not supplied".
            unsafe {
                (self.collect_auxiliary)(
                    values.as_ptr(),
                    values.len() as u32,
                    window.start_index,
                    window.stop_index,
                    window.low16_stride,
                    window.draw_index,
                    spec.playthrough,
                    spec.mode_threshold,
                    spec.filtered_terrain_rows.as_ptr(),
                    spec.filtered_terrain_rows.len() as u32,
                    spec.terrain_row_count,
                    spec.allowed_terrain_rows.as_ptr(),
                    u32::from(spec.has_terrain_constraint),
                    spec.descriptor_thresholds.as_ptr(),
                    spec.selector_threshold,
                    spec.role_five_threshold,
                    spec.selector_value,
                    spec.enemy_rows.as_ptr().cast::<c_void>(),
                    spec.enemy_rows.len() as u32 / super::ENEMY_ROW_BYTES as u32,
                    spec.terrains.as_ptr().cast::<c_void>(),
                    spec.terrains.len() as u32 / super::TERRAIN_ROW_BYTES as u32,
                    spec.contexts.as_ptr().cast::<c_void>(),
                    spec.contexts.len() as u32 / super::CONTEXT_ROW_BYTES as u32,
                    enemy_keys_ptr,
                    enemy_key_count,
                    enemy_offsets_ptr,
                    spec.enemy_group_count,
                    spec.scratch_group_count,
                    if spec.rule_rows.is_empty() {
                        std::ptr::null()
                    } else {
                        rule_rows.cast::<c_void>()
                    },
                    spec.rule_rows.len() as u32 / super::RULE_ROW_BYTES as u32,
                    rule_keys_ptr,
                    rule_key_count,
                    rule_offsets_ptr,
                    spec.rule_criterion_groups.len() as u32,
                    seeds.as_mut_ptr(),
                    trials.as_mut_ptr(),
                    capacity,
                    stage_counts.as_mut_ptr(),
                    stage_counts.len() as u32,
                )
            }
        }

        /// `generate_terrain_row_indices` over one bounded seed batch.
        pub(super) fn terrain_row_indices(
            &self,
            seeds: &[u32],
            mode_threshold: i32,
            filtered_rows: &[u32],
            terrain_row_count: u32,
            output: &mut [u32],
        ) -> i32 {
            debug_assert_eq!(seeds.len(), output.len());
            // SAFETY: every pointer refers to a live slice and the caller
            // validated the row table against the native ABI.
            unsafe {
                (self.terrain_rows)(
                    seeds.as_ptr(),
                    seeds.len() as u64,
                    mode_threshold,
                    filtered_rows.as_ptr(),
                    filtered_rows.len() as u32,
                    terrain_row_count,
                    output.as_mut_ptr(),
                )
            }
        }

        /// `match_enemy_constraints` over one bounded seed batch.
        #[allow(clippy::too_many_arguments)]
        pub(super) fn match_enemy_constraints(
            &self,
            seeds: &[u32],
            terrain_rows: &[u32],
            playthrough: u8,
            mode_threshold: i32,
            descriptor_thresholds: &[i32; 3],
            selector_threshold: i32,
            role_five_threshold: i32,
            selector_value: u8,
            enemy_rows: &[u8],
            terrains: &[u8],
            contexts: &[u8],
            groups: &[Vec<u32>],
            output: &mut [u32],
        ) -> i32 {
            let (keys, offsets) = flatten_groups(groups).unwrap_or_default();
            let (keys_ptr, key_count) = raw_u32(&keys);
            let (offsets_ptr, _) = raw_u16(&offsets);
            debug_assert_eq!(seeds.len(), output.len());
            // SAFETY: the packed blobs use the 18/5/22-byte native ABI and were
            // validated by the caller; the slices outlive the call.
            unsafe {
                (self.match_enemy)(
                    seeds.as_ptr(),
                    terrain_rows.as_ptr(),
                    seeds.len() as u64,
                    playthrough,
                    mode_threshold,
                    descriptor_thresholds.as_ptr(),
                    selector_threshold,
                    role_five_threshold,
                    selector_value,
                    enemy_rows.as_ptr().cast::<c_void>(),
                    (enemy_rows.len() / super::ENEMY_ROW_BYTES) as u32,
                    terrains.as_ptr().cast::<c_void>(),
                    (terrains.len() / super::TERRAIN_ROW_BYTES) as u32,
                    contexts.as_ptr().cast::<c_void>(),
                    (contexts.len() / super::CONTEXT_ROW_BYTES) as u32,
                    keys_ptr,
                    key_count,
                    offsets_ptr,
                    groups.len() as u32,
                    output.as_mut_ptr(),
                )
            }
        }

        /// `match_special_rule_constraints` over one bounded seed batch.
        pub(super) fn match_special_rule_constraints(
            &self,
            seeds: &[u32],
            scratch_masks: &[u32],
            rule_rows: &[u8],
            groups: &[Vec<u16>],
            output: &mut [u32],
        ) -> i32 {
            let (keys, offsets) = flatten_groups(groups).unwrap_or_default();
            let (keys_ptr, key_count) = raw_u16(&keys);
            let (offsets_ptr, _) = raw_u16(&offsets);
            debug_assert_eq!(seeds.len(), output.len());
            // SAFETY: packed rule rows use the 16-byte native ABI validated by
            // the caller; every pointer refers to a live slice.
            unsafe {
                (self.match_rules)(
                    seeds.as_ptr(),
                    scratch_masks.as_ptr(),
                    seeds.len() as u64,
                    rule_rows.as_ptr().cast::<c_void>(),
                    (rule_rows.len() / super::RULE_ROW_BYTES) as u32,
                    keys_ptr,
                    key_count,
                    offsets_ptr,
                    groups.len() as u32,
                    output.as_mut_ptr(),
                )
            }
        }

        /// `generate_ng3_primary_effect_ids_context` over one bounded batch.
        pub(super) fn primary_effect_ids(
            &self,
            seeds: &[u32],
            spec: &PrimaryEffectSpec,
            output: &mut [u32],
        ) -> i32 {
            debug_assert_eq!(seeds.len(), output.len());
            // SAFETY: every pointer refers to a live slice; both lookups hold
            // 65,536 u32 entries and the caller validated the context fields
            // against the exported ABI before this call.
            unsafe {
                (self.primary_ids)(
                    seeds.as_ptr(),
                    seeds.len() as u64,
                    spec.normal_lookup.as_ptr(),
                    spec.promoted_lookup.as_ptr(),
                    spec.promotion_success_lookup.as_ptr(),
                    spec.random7_lookup.as_ptr(),
                    spec.pre_promotion_draws,
                    spec.slot_limit,
                    spec.excluded_slot_mask,
                    spec.primary_source_index,
                    output.as_mut_ptr(),
                )
            }
        }
    }

    fn raw_u32(values: &[u32]) -> (*const u32, u32) {
        if values.is_empty() {
            (std::ptr::null(), 0)
        } else {
            (values.as_ptr(), values.len() as u32)
        }
    }

    fn raw_u16(values: &[u16]) -> (*const u16, u32) {
        if values.is_empty() {
            (std::ptr::null(), 0)
        } else {
            (values.as_ptr(), values.len() as u32)
        }
    }

    fn little_endian_u32(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    }

    /// Resolve one exported symbol into a typed function pointer.
    unsafe fn symbol<T: Copy>(handle: *mut c_void, name: &[u8]) -> Option<T> {
        let address = unsafe { GetProcAddress(handle, name.as_ptr()) };
        if address.is_null() {
            return None;
        }
        Some(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&address) })
    }
}

#[cfg(not(windows))]
mod platform {
    use std::path::Path;

    use super::{AuxiliaryPivotSpec, PivotWindow, PrimaryEffectSpec, R4PrimaryPivotSpec};

    /// Non-Windows stub: the shipped accelerator is a Windows DLL, so every
    /// load fails and no native call is ever made.
    pub(super) struct Native;

    impl Native {
        pub(super) fn load(_module_path: &Path) -> Option<Self> {
            None
        }

        pub(super) fn build_id(&self) -> String {
            String::new()
        }

        pub(super) fn set_policy(&self, _policy: i32) -> i32 {
            -1
        }

        pub(super) fn cuda_available(&self) -> bool {
            false
        }

        pub(super) fn last_backend(&self) -> i32 {
            -1
        }

        pub(super) fn last_cuda_error(&self) -> i32 {
            0
        }

        pub(super) fn last_cuda_stage(&self) -> i32 {
            0
        }

        pub(super) fn force_cuda_failure(&self, _enabled: bool) {}

        pub(super) fn bulk_cpu_call_count(&self) -> u64 {
            0
        }

        pub(super) fn build_weighted_lookup(
            &self,
            _effect_ids: &[u32],
            _weights: &[u32],
            _output: &mut [u32],
        ) -> i32 {
            -1
        }

        pub(super) fn collect_natural_pivot_seeds(
            &self,
            _values: &[u16],
            _window: PivotWindow,
            _seeds: &mut [u32],
            _trials: &mut [u64],
            _capacity: u64,
        ) -> u64 {
            super::ERROR_RESULT
        }

        pub(super) fn collect_r4_primary_pivot_seeds(
            &self,
            _values: &[u16],
            _spec: &R4PrimaryPivotSpec,
            _window: PivotWindow,
            _seeds: &mut [u32],
            _trials: &mut [u64],
            _capacity: u64,
        ) -> u64 {
            super::ERROR_RESULT
        }

        pub(super) fn collect_auxiliary_pivot_matches(
            &self,
            _values: &[u16],
            _spec: &AuxiliaryPivotSpec,
            _window: PivotWindow,
            _seeds: &mut [u32],
            _trials: &mut [u64],
            _capacity: u64,
            _stage_counts: &mut [u64],
        ) -> u64 {
            super::ERROR_RESULT
        }

        pub(super) fn terrain_row_indices(
            &self,
            _seeds: &[u32],
            _mode_threshold: i32,
            _filtered_rows: &[u32],
            _terrain_row_count: u32,
            _output: &mut [u32],
        ) -> i32 {
            -1
        }

        pub(super) fn match_enemy_constraints(
            &self,
            _seeds: &[u32],
            _terrain_rows: &[u32],
            _playthrough: u8,
            _mode_threshold: i32,
            _descriptor_thresholds: &[i32; 3],
            _selector_threshold: i32,
            _role_five_threshold: i32,
            _selector_value: u8,
            _enemy_rows: &[u8],
            _terrains: &[u8],
            _contexts: &[u8],
            _groups: &[Vec<u32>],
            _output: &mut [u32],
        ) -> i32 {
            -1
        }

        pub(super) fn match_special_rule_constraints(
            &self,
            _seeds: &[u32],
            _scratch_masks: &[u32],
            _rule_rows: &[u8],
            _groups: &[Vec<u16>],
            _output: &mut [u32],
        ) -> i32 {
            -1
        }

        pub(super) fn primary_effect_ids(
            &self,
            _seeds: &[u32],
            _spec: &PrimaryEffectSpec,
            _output: &mut [u32],
        ) -> i32 {
            -1
        }
    }
}

/// Execution-policy consistency between a load probe and an operation guard.
///
/// The release failure this covers was silent: a probe wrote strict GPU
/// straight into the loaded library while the lock's mirror still advertised
/// the operation's opt-in, so the mirror alone cannot prove the invariant. The
/// first test drives a controllable setter and the second observes the library
/// itself through a real call.
#[cfg(all(test, windows))]
mod policy_consistency_tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use super::{
        Accelerator, ExecutionPolicy, NativeBackend, NativeSearchError, PivotWindow,
        EXECUTION_POLICY_ALLOW_BULK_CPU, EXECUTION_POLICY_STRICT_GPU,
    };

    /// A pivot table and window the native collector serves cheaply.
    const VALUES: [u16; 5] = [0x1234, 0xABCD, 0x0001, 0xFFFE, 0x00FF];
    const STRIDE: u16 = 0x9E37;

    fn window() -> PivotWindow {
        PivotWindow {
            start_index: 0,
            stop_index: 2_000,
            low16_stride: STRIDE,
            draw_index: 1,
        }
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// A private copy of the shipped library.
    ///
    /// The installed policy and the forced-CUDA-failure hook are globals inside
    /// the loaded module, so this test drives its own instance instead of the
    /// one every other test in this crate shares.
    struct PrivateLibrary {
        directory: PathBuf,
        module: PathBuf,
    }

    impl PrivateLibrary {
        fn new() -> Option<Self> {
            let source = repo_root().join("bin").join("nioh3_seed_accelerator.dll");
            if !source.is_file() {
                return None;
            }
            let directory = std::env::temp_dir()
                .join(format!("nioh3-policy-consistency-{}", std::process::id()));
            fs::create_dir_all(&directory).ok()?;
            let module = directory.join("nioh3_seed_accelerator.dll");
            fs::copy(&source, &module).ok()?;
            Some(Self { directory, module })
        }
    }

    impl Drop for PrivateLibrary {
        fn drop(&mut self) {
            // The module stays mapped for the process lifetime, so cleanup is
            // best effort and never a test failure.
            let _ = fs::remove_file(&self.module);
            let _ = fs::remove_dir(&self.directory);
        }
    }

    /// A probe setter must not run between an install and its mirror update.
    #[test]
    fn a_probe_setter_cannot_interleave_an_install() {
        let (entered_tx, entered_rx) = mpsc::channel::<()>();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let (restore_tx, restore_rx) = mpsc::channel::<()>();
        let observed = Arc::new(Mutex::new(None));

        let installer = thread::spawn(move || {
            let previous =
                super::policy_lock::install(EXECUTION_POLICY_ALLOW_BULK_CPU, |_policy| {
                    entered_tx.send(()).expect("signal the install");
                    release_rx.recv().expect("hold the setter open");
                    0
                })
                .expect("the lock installs the opt-in");
            // Hold the guard until the probe has been observed, then restore so
            // the process-global lock is left idle for the rest of the suite.
            restore_rx.recv().expect("hold the guard");
            super::policy_lock::restore(previous, |_policy| 0);
            previous
        });
        entered_rx.recv().expect("the install reached its setter");

        let (probe_tx, probe_rx) = mpsc::channel::<i32>();
        let prober = {
            let observed = Arc::clone(&observed);
            thread::spawn(move || {
                let accepted = super::policy_lock::with_active(|policy| {
                    *observed.lock().expect("probe record") = Some(policy);
                    0
                });
                probe_tx.send(accepted).expect("report the probe");
            })
        };
        assert!(
            probe_rx.recv_timeout(Duration::from_millis(250)).is_err(),
            "a probe wrote the native policy while an install was still in flight"
        );
        release_tx.send(()).expect("release the install");
        assert_eq!(probe_rx.recv().expect("the probe finishes"), 0);
        assert_eq!(
            *observed.lock().expect("probe record"),
            Some(EXECUTION_POLICY_ALLOW_BULK_CPU),
            "the mirror must be updated before the install releases the lock"
        );
        restore_tx.send(()).expect("release the guard");
        assert_eq!(
            installer.join().expect("the installer finishes"),
            EXECUTION_POLICY_STRICT_GPU
        );
        prober.join().expect("the prober finishes");
    }

    /// A concurrent load probe must not cancel an installed opt-in, and the
    /// outermost drop must return the library to strict GPU.
    #[test]
    fn a_load_probe_cannot_cancel_or_leak_the_policy_guard() {
        let _accelerator = crate::accelerator_test_lock();
        let Some(library) = PrivateLibrary::new() else {
            eprintln!("skipping: the shipped seed accelerator is not staged");
            return;
        };
        let root = repo_root();
        let accelerator = Accelerator::load(&root, Some(&library.module))
            .expect("the private copy of the shipped accelerator loads");

        let stop = Arc::new(AtomicBool::new(false));
        let probes = Arc::new(AtomicUsize::new(0));
        let (start_tx, start_rx) = mpsc::channel::<()>();
        let (first_probe_tx, first_probe_rx) = mpsc::channel::<()>();
        let prober = {
            let root = root.clone();
            let module = library.module.clone();
            let stop = Arc::clone(&stop);
            let probes = Arc::clone(&probes);
            thread::spawn(move || {
                start_rx
                    .recv()
                    .expect("start probing while the guard is held");
                let mut first = true;
                while !stop.load(Ordering::Relaxed) {
                    // Both shipped probe paths, each re-installing the policy.
                    let _ = crate::native::probe_seed_accelerator(&root, Some(&module));
                    let _ = Accelerator::load(&root, Some(&module));
                    probes.fetch_add(1, Ordering::Relaxed);
                    if first {
                        first_probe_tx
                            .send(())
                            .expect("acknowledge the first probe");
                        first = false;
                    }
                }
            })
        };

        // A window that runs both shipped probe paths while one opt-in guard is
        // installed, then serves a call only a library holding the bulk-CPU
        // policy can answer.
        let held_window = |root: &Path, module: &Path| {
            let _ = crate::native::probe_seed_accelerator(root, Some(module));
            let _ = Accelerator::load(root, Some(module));
            accelerator.force_cuda_failure(true);
            let served = accelerator.collect_natural_pivot_page(&VALUES, window());
            let backend = accelerator.last_backend();
            accelerator.force_cuda_failure(false);
            assert!(
                served.is_ok(),
                "a load probe cancelled the installed opt-in: {:?}",
                served.err()
            );
            assert_eq!(backend, NativeBackend::NativeCpu);
        };

        for iteration in 0..63 {
            let guard = accelerator
                .pin_policy(ExecutionPolicy::AllowBulkCpu)
                .expect("the explicit bulk-CPU opt-in is accepted");
            assert_eq!(accelerator.pinned_policy(), ExecutionPolicy::AllowBulkCpu);
            if iteration == 0 {
                // Thread creation does not guarantee that it has run. Hold the
                // opt-in until both real probe paths have completed on the
                // other thread, even on a busy or single-core runner.
                start_tx.send(()).expect("start the load probe");
                first_probe_rx
                    .recv_timeout(Duration::from_secs(30))
                    .expect("both load probes must finish while the guard is held");
            }
            held_window(&root, &library.module);
            drop(guard);
        }
        // The final window keeps its guard installed until the prober has been
        // stopped, so the last write to the private library is that guard's own
        // restore rather than a probe racing the drop.
        let guard = accelerator
            .pin_policy(ExecutionPolicy::AllowBulkCpu)
            .expect("the explicit bulk-CPU opt-in is accepted");
        assert_eq!(accelerator.pinned_policy(), ExecutionPolicy::AllowBulkCpu);
        held_window(&root, &library.module);
        stop.store(true, Ordering::Relaxed);
        prober.join().expect("the prober finishes");
        assert!(probes.load(Ordering::Relaxed) > 0, "no load probe ran");
        drop(guard);

        // The outermost drop restores strict GPU, again observed by a real call.
        // Only this test writes this private copy, and its probes stopped above,
        // so the library itself is the evidence: the mirror is not enough.
        accelerator.force_cuda_failure(true);
        let strict = accelerator.collect_natural_pivot_page(&VALUES, window());
        accelerator.force_cuda_failure(false);
        assert!(
            matches!(strict, Err(NativeSearchError::CudaUnavailable { .. })),
            "the strict GPU default was not restored: {strict:?}"
        );
    }

    /// The lock is re-entrant per thread and wakes one waiter per release.
    #[test]
    fn the_policy_lock_nests_and_releases_by_owner() {
        let outer = super::policy_lock::install(EXECUTION_POLICY_ALLOW_BULK_CPU, |_policy| 0)
            .expect("the outer opt-in installs");
        assert_eq!(outer, EXECUTION_POLICY_STRICT_GPU);
        let inner = super::policy_lock::install(EXECUTION_POLICY_STRICT_GPU, |_policy| 0)
            .expect("a nested strict pin installs");
        assert_eq!(inner, EXECUTION_POLICY_ALLOW_BULK_CPU);
        assert_eq!(
            super::policy_lock::active(),
            EXECUTION_POLICY_STRICT_GPU,
            "the mirror must follow the innermost guard"
        );
        super::policy_lock::restore(inner, |_policy| 0);
        assert_eq!(
            super::policy_lock::active(),
            EXECUTION_POLICY_ALLOW_BULK_CPU,
            "dropping the inner guard must restore the outer policy"
        );
        // A rejected policy must not take the lock or change the mirror.
        assert!(
            super::policy_lock::install(EXECUTION_POLICY_ALLOW_BULK_CPU, |_policy| -1).is_err()
        );
        assert_eq!(
            super::policy_lock::active(),
            EXECUTION_POLICY_ALLOW_BULK_CPU,
            "a rejected install must leave the lock untouched"
        );
        super::policy_lock::restore(outer, |_policy| 0);
        // This thread no longer holds the lock, so the next acquisition is a
        // fresh one and reports the strict GPU default it installs from.
        let fresh = super::policy_lock::install(EXECUTION_POLICY_ALLOW_BULK_CPU, |_policy| 0)
            .expect("the lock is free after the outermost drop");
        assert_eq!(fresh, EXECUTION_POLICY_STRICT_GPU);
        super::policy_lock::restore(fresh, |_policy| 0);
    }
}
