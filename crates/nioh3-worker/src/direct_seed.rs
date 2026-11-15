//! Bounded DirectCompute seed operations over the same cursor and packed tables
//! as the CUDA ABI. Shader hits remain subject to the existing certified replay.
use crate::native_search::{
    AuxiliaryPivotPage, AuxiliaryPivotSpec, NativeBackend, NativeSearchError, PivotMatch,
    PivotWindow, PrimaryEffectSpec, R4PrimaryPivotSpec,
};
use std::path::Path;

/// A diagnostic preference never authorizes bulk CPU execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedBackendPreference {
    Auto,
    Cuda,
    DirectCompute,
}
impl SeedBackendPreference {
    pub fn from_environment() -> Option<Self> {
        match std::env::var("NIOH3_SEED_BACKEND")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "" | "auto" => Some(Self::Auto),
            "cuda" => Some(Self::Cuda),
            "directcompute" => Some(Self::DirectCompute),
            _ => None,
        }
    }
}

/// The only legal dispatch outcomes for one operation-scoped policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedBackendChoice {
    Cuda,
    DirectCompute,
    CpuOptIn,
    RefusedCuda,
    RefusedDirectCompute,
}
impl SeedBackendPreference {
    pub fn select(
        self,
        cuda: bool,
        directcompute: bool,
        policy: crate::native_search::ExecutionPolicy,
    ) -> SeedBackendChoice {
        use crate::native_search::ExecutionPolicy;
        use SeedBackendChoice::{CpuOptIn, Cuda, DirectCompute, RefusedCuda, RefusedDirectCompute};
        match self {
            Self::Auto => match (cuda, directcompute, policy) {
                (true, _, _) => Cuda,
                (false, true, _) => DirectCompute,
                (false, false, ExecutionPolicy::AllowBulkCpu) => CpuOptIn,
                (false, false, ExecutionPolicy::StrictGpu) => RefusedCuda,
            },
            Self::Cuda => match (cuda, policy) {
                (true, _) => Cuda,
                (false, ExecutionPolicy::AllowBulkCpu) => CpuOptIn,
                (false, ExecutionPolicy::StrictGpu) => RefusedCuda,
            },
            Self::DirectCompute => match directcompute {
                true => DirectCompute,
                false => RefusedDirectCompute,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
enum Operation {
    Natural = 0,
    R4Primary = 1,
    Auxiliary = 2,
    PrimaryFilter = 3,
    AuxiliaryFilter = 4,
}

struct Packet {
    words: Vec<u32>,
}
impl Packet {
    fn new(operation: Operation) -> Self {
        let mut words = vec![0; 48];
        words[0] = operation as u32;
        Self { words }
    }
    fn array(&mut self, slot: usize, values: impl IntoIterator<Item = u32>) {
        self.words[slot] = self.words.len() as u32;
        self.words.extend(values);
    }
    fn pivot(&mut self, values: &[u16], window: PivotWindow) {
        self.array(1, values.iter().copied().map(u32::from));
        self.words[2] = values.len() as u32;
        self.words[3] = u32::from(window.low16_stride);
        self.words[4] = window.draw_index;
    }
    fn auxiliary(&mut self, spec: &AuxiliaryPivotSpec) -> Result<(), NativeSearchError> {
        self.words[7] = spec.mode_threshold as u32;
        self.array(8, spec.filtered_terrain_rows.iter().copied());
        self.words[9] = spec.filtered_terrain_rows.len() as u32;
        self.words[10] = spec.terrain_row_count;
        self.array(11, spec.allowed_terrain_rows.iter().copied().map(u32::from));
        self.words[12] = u32::from(spec.playthrough);
        self.array(13, spec.descriptor_thresholds.map(|v| v as u32));
        self.words[14] = spec.selector_threshold as u32;
        self.words[15] = spec.role_five_threshold as u32;
        self.words[16] = u32::from(spec.selector_value);
        let mut enemies = Vec::new();
        for row in spec.enemy_rows.as_chunks::<18>().0.iter() {
            enemies.extend([
                u32_at(row, 0),
                u32_at(row, 4),
                u16_at(row, 8),
                u16_at(row, 10),
            ]);
            enemies.extend(row[12..18].iter().copied().map(u32::from));
        }
        self.array(17, enemies);
        self.words[18] = (spec.enemy_rows.len() / 18) as u32;
        self.array(
            19,
            spec.terrains
                .as_chunks::<5>()
                .0
                .iter()
                .flat_map(|r| [u16_at(r, 0), u16_at(r, 2), u32::from(r[4])]),
        );
        self.words[20] = (spec.terrains.len() / 5) as u32;
        let mut contexts = Vec::new();
        for row in spec.contexts.as_chunks::<22>().0.iter() {
            contexts.extend([u32::from(row[0]), u32::from(row[1])]);
            contexts.extend((0..5).map(|i| u32_at(row, 2 + i * 4)));
        }
        self.array(21, contexts);
        self.words[22] = (spec.contexts.len() / 22) as u32;
        let (keys, offsets) = crate::native_search::flatten_groups(&spec.enemy_criterion_groups)?;
        self.array(23, keys);
        self.array(24, offsets.into_iter().map(u32::from));
        self.words[25] = spec.enemy_group_count;
        self.words[26] = spec.scratch_group_count;
        self.array(
            27,
            spec.rule_rows.as_chunks::<16>().0.iter().flat_map(|r| {
                [
                    u16_at(r, 0),
                    u16_at(r, 2),
                    u32_at(r, 4),
                    u16_at(r, 8),
                    u16_at(r, 10),
                    u32::from(r[12]),
                    u32::from(r[13]),
                    u16_at(r, 14),
                ]
            }),
        );
        self.words[28] = (spec.rule_rows.len() / 16) as u32;
        let (keys, offsets) = crate::native_search::flatten_groups(&spec.rule_criterion_groups)?;
        self.array(29, keys.into_iter().map(u32::from));
        self.array(30, offsets.into_iter().map(u32::from));
        self.words[31] = spec.rule_criterion_groups.len() as u32;
        self.words[32] = u32::from(spec.has_terrain_constraint);
        Ok(())
    }
}
fn u16_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]))
}
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated packed field"),
    )
}

/// The effect helper owns these extra exports; older helpers are unavailable,
/// never mistaken for a seed-capable DirectCompute backend.
pub struct DirectSeed {
    native: platform::Native,
    vendor: u32,
}
impl DirectSeed {
    pub fn load(root: &Path) -> Option<Self> {
        let path = crate::capabilities::effect_preimage_path(root, None);
        // Reuse the product's byte-identity check before resolving new exports.
        crate::preimage::PreimageAccelerator::load(root, Some(&path)).ok()?;
        let native = platform::Native::load(&path)?;
        Some(Self {
            native,
            vendor: crate::preimage::configured_vendor_id(),
        })
    }
    pub fn available(&self) -> bool {
        self.native.available(self.vendor)
    }
    pub fn natural(
        &self,
        values: &[u16],
        window: PivotWindow,
    ) -> Result<Vec<PivotMatch>, NativeSearchError> {
        let mut packet = Packet::new(Operation::Natural);
        packet.pivot(values, window);
        let (pairs, _) = self.execute(
            &packet,
            window.start_index,
            window.stop_index,
            (window.stop_index - window.start_index) as usize,
            0,
        )?;
        Ok(matches(pairs))
    }
    pub fn r4_primary(
        &self,
        values: &[u16],
        spec: &R4PrimaryPivotSpec,
        window: PivotWindow,
        capacity: usize,
    ) -> Result<Vec<PivotMatch>, NativeSearchError> {
        if spec
            .context_by_first_u16
            .iter()
            .any(|v| u32::from(*v) >= spec.context_count)
        {
            return Err(NativeSearchError::InvalidInput(
                "R4 context index exceeds its lookup matrices",
            ));
        }
        let mut packet = Packet::new(Operation::R4Primary);
        packet.pivot(values, window);
        packet.array(33, spec.context_by_first_u16.iter().copied().map(u32::from));
        packet.array(
            34,
            spec.normal_lookups
                .as_chunks::<4>()
                .0
                .iter()
                .map(|r| u32_at(r, 0)),
        );
        packet.array(
            35,
            spec.promoted_lookups
                .as_chunks::<4>()
                .0
                .iter()
                .map(|r| u32_at(r, 0)),
        );
        packet.array(
            36,
            spec.promotion_success_lookup.iter().copied().map(u32::from),
        );
        packet.array(37, spec.random7_lookup.iter().copied().map(u32::from));
        packet.array(38, spec.allowed_effect_ids.iter().copied());
        packet.words[39] = spec.allowed_effect_ids.len() as u32;
        let (pairs, _) =
            self.execute(&packet, window.start_index, window.stop_index, capacity, 0)?;
        Ok(matches(pairs))
    }
    pub fn auxiliary(
        &self,
        values: &[u16],
        spec: &AuxiliaryPivotSpec,
        window: PivotWindow,
        capacity: usize,
    ) -> Result<AuxiliaryPivotPage, NativeSearchError> {
        let mut packet = Packet::new(Operation::Auxiliary);
        packet.pivot(values, window);
        packet.auxiliary(spec)?;
        let (pairs, stage_counts) = self.execute(
            &packet,
            window.start_index,
            window.stop_index,
            capacity,
            spec.stage_count(),
        )?;
        Ok(AuxiliaryPivotPage {
            matches: matches(pairs),
            stage_counts,
            backend: NativeBackend::DirectCompute,
        })
    }
    pub fn primary_selected(
        &self,
        spec: &PrimaryEffectSpec,
        seeds: &[u32],
    ) -> Result<Vec<bool>, NativeSearchError> {
        let mut packet = Packet::new(Operation::PrimaryFilter);
        packet.array(5, seeds.iter().copied());
        packet.array(34, spec.normal_lookup.iter().copied());
        packet.array(35, spec.promoted_lookup.iter().copied());
        packet.array(
            36,
            spec.promotion_success_lookup.iter().copied().map(u32::from),
        );
        packet.array(37, spec.random7_lookup.iter().copied().map(u32::from));
        packet.words[40] = spec.pre_promotion_draws;
        packet.words[41] = u32::from(spec.slot_limit);
        packet.words[42] = u32::from(spec.excluded_slot_mask);
        packet.words[43] = u32::from(spec.primary_source_index);
        let (pairs, _) = self.execute(&packet, 0, seeds.len() as u64, seeds.len(), 0)?;
        Ok(pairs
            .into_iter()
            .map(|pair| spec.allowed_effect_ids.contains(&pair[0]))
            .collect())
    }
    pub fn auxiliary_selected(
        &self,
        spec: &AuxiliaryPivotSpec,
        seeds: &[u32],
    ) -> Result<Vec<bool>, NativeSearchError> {
        let mut packet = Packet::new(Operation::AuxiliaryFilter);
        packet.array(5, seeds.iter().copied());
        packet.auxiliary(spec)?;
        let (pairs, _) = self.execute(&packet, 0, seeds.len() as u64, seeds.len(), 0)?;
        Ok(pairs.into_iter().map(|pair| pair[0] != 0).collect())
    }
    fn execute(
        &self,
        packet: &Packet,
        start: u64,
        stop: u64,
        capacity: usize,
        stage_count: usize,
    ) -> Result<(Vec<[u32; 2]>, Vec<u64>), NativeSearchError> {
        if start == stop {
            return Ok((Vec::new(), vec![0; stage_count]));
        }
        self.native.execute(
            &packet.words,
            start,
            stop,
            capacity,
            stage_count,
            self.vendor,
        )
    }
}
fn matches(mut pairs: Vec<[u32; 2]>) -> Vec<PivotMatch> {
    pairs.sort_unstable_by_key(|pair| pair[1]);
    pairs
        .into_iter()
        .map(|pair| PivotMatch {
            seed: pair[0],
            trial: u64::from(pair[1]) + 1,
        })
        .collect()
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }
    type Probe = unsafe extern "C" fn(u32) -> i32;
    #[allow(clippy::type_complexity)]
    type Execute = unsafe extern "C" fn(
        *const u32,
        u32,
        u64,
        u64,
        u32,
        *mut u32,
        *mut u64,
        u32,
        u32,
        *mut u32,
        *mut i32,
    ) -> u64;
    pub(super) struct Native {
        _handle: *mut c_void,
        probe: Probe,
        execute: Execute,
    }
    // SAFETY: modules live for the process lifetime; the helper serializes its
    // device/context state and the job owner pins the CPU execution policy.
    unsafe impl Send for Native {}
    // SAFETY: see Send; no handle can be unloaded during an operation.
    unsafe impl Sync for Native {}
    impl Native {
        pub(super) fn load(path: &Path) -> Option<Self> {
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            // SAFETY: the terminated path outlives the call; exports use ABI 1.
            unsafe {
                let handle = LoadLibraryW(wide.as_ptr());
                if handle.is_null() {
                    return None;
                }
                let abi: unsafe extern "C" fn() -> i32 =
                    symbol(handle, b"d3d11_seed_search_abi_version\0")?;
                if abi() != 1 {
                    return None;
                }
                Some(Self {
                    _handle: handle,
                    probe: symbol(handle, b"d3d11_seed_search_available\0")?,
                    execute: symbol(handle, b"execute_seed_search_d3d11\0")?,
                })
            }
        }
        pub(super) fn available(&self, vendor: u32) -> bool {
            // SAFETY: the resolved read-only self-test export takes a vendor id.
            unsafe { (self.probe)(vendor) != 0 }
        }
        pub(super) fn execute(
            &self,
            words: &[u32],
            start: u64,
            stop: u64,
            capacity: usize,
            stage_count: usize,
            vendor: u32,
        ) -> Result<(Vec<[u32; 2]>, Vec<u64>), NativeSearchError> {
            let mut pairs = vec![[0; 2]; capacity];
            let mut stages = vec![0; stage_count];
            let mut used_vendor = 0;
            let mut error = 0;
            // SAFETY: all buffers remain live and their exact capacities are
            // passed to the bounded ABI. The helper rejects output overflow.
            let count = unsafe {
                (self.execute)(
                    words.as_ptr(),
                    words.len() as u32,
                    start,
                    stop,
                    capacity as u32,
                    pairs.as_mut_ptr().cast(),
                    stages.as_mut_ptr(),
                    stage_count as u32,
                    vendor,
                    &mut used_vendor,
                    &mut error,
                )
            };
            if count == u64::MAX || count > capacity as u64 {
                return Err(NativeSearchError::DirectComputeUnavailable(format!(
                    "seed operation {} failed on vendor 0x{used_vendor:04X} (HRESULT 0x{:08X})",
                    words[0], error as u32
                )));
            }
            pairs.truncate(count as usize);
            Ok((pairs, stages))
        }
    }
    unsafe fn symbol<T: Copy>(handle: *mut c_void, name: &[u8]) -> Option<T> {
        let address = unsafe { GetProcAddress(handle, name.as_ptr()) };
        if address.is_null() {
            None
        } else {
            Some(unsafe { std::mem::transmute_copy(&address) })
        }
    }
}
#[cfg(not(windows))]
mod platform {
    use super::*;
    pub(super) struct Native;
    impl Native {
        pub(super) fn load(_path: &Path) -> Option<Self> {
            None
        }
        pub(super) fn available(&self, _vendor: u32) -> bool {
            false
        }
        pub(super) fn execute(
            &self,
            _words: &[u32],
            _start: u64,
            _stop: u64,
            _capacity: usize,
            _stage_count: usize,
            _vendor: u32,
        ) -> Result<(Vec<[u32; 2]>, Vec<u64>), NativeSearchError> {
            Err(NativeSearchError::Unavailable)
        }
    }
}
