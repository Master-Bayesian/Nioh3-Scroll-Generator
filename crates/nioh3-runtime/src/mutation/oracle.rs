//! The native batch oracle: the shipped `native.NativeBatchOracle`, ported.
//!
//! This is the primitive `runtime.generate`, `runtime.search` and
//! `runtime.capture_grace` all sit on. It runs one of the five shipped remote
//! wrappers inside the game process and reads the records it produced:
//!
//! * `generate`: the canonicalize loop over a batch of source records;
//! * `generate_seed_range`: the same loop driving the seed, with or without an
//!   explicit playthrough context;
//! * `finalize_effect_stage` / `finalize_effect_stage_batch`: one completion
//!   index over one record or a contiguous batch;
//! * `finalize_stage_record` / `finalize_stage_records_batch`: the game's own
//!   outer completion loop, which never feeds a failed attempt into the next
//!   slot.
//!
//! Rights: this module is one of the two callers that legitimately request
//! `PROCESS_CREATE_THREAD`, because `CreateRemoteThread` needs it. The mask is
//! exactly [`crate::mutation::native_abi::RUNTIME_ACCESS`]; `PROCESS_ALL_ACCESS`,
//! termination, suspension and debugger entry points are never requested here.
//! A timed-out call is retired to a waiter that frees the allocation only after
//! it observes the thread exit, so `remote_call_pending` never lies.

use crate::error::RuntimeError;
use crate::mutation::native_abi::{
    build_batch_wrapper, build_effect_finalizer_batch_wrapper, build_effect_finalizer_wrapper,
    build_explicit_playthrough_seed_range_wrapper, build_seed_range_wrapper, hex,
    PlaythroughChainRvas, ORACLE_BATCH_LIMIT, REMOTE_CODE_SIZE, SCROLL_RECORD_SIZE,
};
use crate::mutation::win_session::{RemoteSession, WAIT_INFINITE, WAIT_OBJECT_0};
use crate::profile::NativeRuntimeProfile;
use std::sync::{mpsc, Arc, Mutex};

/// The mask a session handed to [`NativeBatchOracle::open`] must hold.
///
/// `CreateRemoteThread` is the one capability the oracle adds to the write
/// mask, and this is the exact shipped value: `PROCESS_CREATE_THREAD |
/// PROCESS_VM_OPERATION | PROCESS_VM_READ | PROCESS_VM_WRITE |
/// PROCESS_QUERY_INFORMATION`. The Oracle never requests
/// `PROCESS_ALL_ACCESS`, a termination right or a suspension right.
pub const ORACLE_ACCESS: u32 = crate::mutation::native_abi::RUNTIME_ACCESS;

const WAIT_TIMEOUT: u32 = 0x0000_0102;

struct RetiredRemoteOwner {
    session: Box<dyn RemoteSession + Send>,
    allocation: u64,
    thread: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RetirementLifecycle {
    Idle,
    ReaperPending,
    RetainedUnsafe,
    Released,
}

struct RetirementState {
    lifecycle: RetirementLifecycle,
    allocation: Option<u64>,
    thread: Option<u64>,
    error: Option<String>,
    owner: Option<RetiredRemoteOwner>,
}

/// A cloneable receipt for one oracle allocation after normal close or timeout.
///
/// The receipt owns any session that could not be handed to a live reaper or
/// whose allocation could not be released. Callers retain it until
/// [`Self::refresh`] proves terminal release; dropping the originating oracle
/// therefore never turns unknown cleanup into `safe_to_shutdown=true`.
#[derive(Clone)]
pub struct OracleRetirement {
    inner: Arc<Mutex<RetirementState>>,
}

/// Read-only ownership facts for diagnostics and shutdown decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleRetirementSnapshot {
    pub lifecycle: &'static str,
    pub allocation: Option<u64>,
    pub thread: Option<u64>,
    pub error: Option<String>,
    pub pending_remote_call: bool,
    pub safe_to_shutdown: bool,
}

impl OracleRetirement {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RetirementState {
                lifecycle: RetirementLifecycle::Idle,
                allocation: None,
                thread: None,
                error: None,
                owner: None,
            })),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, RetirementState> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn mark_reaper_pending(&self, allocation: u64, thread: u64) {
        let mut state = self.lock();
        state.lifecycle = RetirementLifecycle::ReaperPending;
        state.allocation = Some(allocation);
        state.thread = Some(thread);
        state.error = None;
        state.owner = None;
    }

    fn retain(&self, owner: RetiredRemoteOwner, error: String) {
        let mut state = self.lock();
        state.lifecycle = RetirementLifecycle::RetainedUnsafe;
        state.allocation = Some(owner.allocation);
        state.thread = owner.thread;
        state.error = Some(error);
        state.owner = Some(owner);
    }

    fn release(&self) {
        let mut state = self.lock();
        state.lifecycle = RetirementLifecycle::Released;
        state.allocation = None;
        state.thread = None;
        state.error = None;
        state.owner = None;
    }

    /// Current ownership facts without performing process work.
    pub fn snapshot(&self) -> OracleRetirementSnapshot {
        let state = self.lock();
        let safe = matches!(
            state.lifecycle,
            RetirementLifecycle::Idle | RetirementLifecycle::Released
        );
        OracleRetirementSnapshot {
            lifecycle: match state.lifecycle {
                RetirementLifecycle::Idle => "idle",
                RetirementLifecycle::ReaperPending => "reaper_pending",
                RetirementLifecycle::RetainedUnsafe => "retained_unsafe",
                RetirementLifecycle::Released => "released",
            },
            allocation: state.allocation,
            thread: state.thread,
            error: state.error.clone(),
            pending_remote_call: !safe,
            safe_to_shutdown: safe,
        }
    }

    /// Retry retained cleanup without blocking on an incomplete remote thread.
    pub fn refresh(&self) -> bool {
        let owner = {
            let mut state = self.lock();
            if state.lifecycle != RetirementLifecycle::RetainedUnsafe {
                return matches!(
                    state.lifecycle,
                    RetirementLifecycle::Idle | RetirementLifecycle::Released
                );
            }
            state.owner.take()
        };
        let Some(mut owner) = owner else {
            return false;
        };

        if let Some(thread) = owner.thread {
            match owner.session.wait_thread(thread, 0) {
                Ok(WAIT_OBJECT_0) => {
                    owner.session.close_thread(thread);
                    owner.thread = None;
                }
                Ok(WAIT_TIMEOUT) => {
                    self.retain(owner, "remote call is still running".to_string());
                    return false;
                }
                Ok(waited) => {
                    self.retain(
                        owner,
                        format!("unexpected remote-thread wait result: {waited:#x}"),
                    );
                    return false;
                }
                Err(error) => {
                    self.retain(owner, format!("remote-thread wait failed: {error}"));
                    return false;
                }
            }
        }

        match owner.session.free(owner.allocation) {
            Ok(()) => {
                owner.session.close();
                self.release();
                true
            }
            Err(error) => {
                self.retain(owner, format!("remote allocation release failed: {error}"));
                false
            }
        }
    }
}

fn finish_retired_owner(mut owner: RetiredRemoteOwner, retirement: OracleRetirement) {
    let Some(thread) = owner.thread else {
        retirement.retain(owner, "retired remote call has no thread owner".to_string());
        return;
    };
    match owner.session.wait_thread(thread, WAIT_INFINITE) {
        Ok(WAIT_OBJECT_0) => {
            owner.session.close_thread(thread);
            owner.thread = None;
        }
        Ok(waited) => {
            retirement.retain(
                owner,
                format!("unexpected remote-thread wait result: {waited:#x}"),
            );
            return;
        }
        Err(error) => {
            retirement.retain(owner, format!("remote-thread wait failed: {error}"));
            return;
        }
    }
    match owner.session.free(owner.allocation) {
        Ok(()) => {
            owner.session.close();
            retirement.release();
        }
        Err(error) => {
            retirement.retain(owner, format!("remote allocation release failed: {error}"));
        }
    }
}

/// `emaki_exchange.EFFECT_START + index * EFFECT_STRIDE + 0x0E`.
fn effect_flag_offset(index: usize) -> usize {
    0x34 + index * 0x18 + 0x0E
}

/// `emaki_exchange.EFFECT_START + index * EFFECT_STRIDE + 0x0D`.
fn effect_word_offset(index: usize) -> usize {
    0x34 + index * 0x18 + 0x0D
}

/// The native batch oracle.
pub struct NativeBatchOracle {
    session: Option<Box<dyn RemoteSession + Send>>,
    pid: u32,
    pub module_base: u64,
    profile: NativeRuntimeProfile,
    max_batch_size: usize,
    preserve_requested_rarity: bool,
    allocation: Option<u64>,
    pub source_address: u64,
    pub destination_address: u64,
    retirement: OracleRetirement,
}

impl NativeBatchOracle {
    /// `NativeBatchOracle.__init__` without the process handle.
    pub fn new(
        pid: u32,
        module_base: u64,
        profile: NativeRuntimeProfile,
        max_batch_size: usize,
        preserve_requested_rarity: bool,
    ) -> Result<Self, RuntimeError> {
        if max_batch_size == 0 || max_batch_size > ORACLE_BATCH_LIMIT {
            return Err(RuntimeError::OracleRejected {
                detail: "max_batch_size must be between 1 and 4096".to_string(),
            });
        }
        Ok(Self {
            session: None,
            pid,
            module_base,
            profile,
            max_batch_size,
            preserve_requested_rarity,
            allocation: None,
            source_address: 0,
            destination_address: 0,
            retirement: OracleRetirement::new(),
        })
    }

    /// `NativeBatchOracle.open`: verify every signature, then allocate the one
    /// code-plus-buffers region the batch calls share.
    pub fn open(&mut self, session: Box<dyn RemoteSession + Send>) -> Result<(), RuntimeError> {
        if self.session.is_some() {
            return Ok(());
        }
        let mut session = session;
        let base = self.module_base;
        for site in self.profile.verification_sites() {
            if site.signature.is_empty() {
                return Err(RuntimeError::ProfileMissingSignature {
                    site: site.name.to_string(),
                });
            }
            let actual = session.read(base + site.rva, site.signature.len())?;
            if actual != site.signature {
                return Err(RuntimeError::SignatureMismatch {
                    site: site.name.to_string(),
                    rva: site.rva,
                });
            }
        }
        let source_size = self.max_batch_size * SCROLL_RECORD_SIZE;
        let allocation_size = REMOTE_CODE_SIZE as usize + source_size * 2;
        let allocation = session.allocate(allocation_size)?;
        self.source_address = allocation + REMOTE_CODE_SIZE;
        self.destination_address = self.source_address + source_size as u64;
        self.allocation = Some(allocation);
        self.session = Some(session);
        Ok(())
    }

    pub fn close(&mut self) {
        if let (Some(mut session), Some(allocation)) = (self.session.take(), self.allocation.take())
        {
            match session.free(allocation) {
                Ok(()) => {
                    session.close();
                    self.retirement.release();
                }
                Err(error) => self.retirement.retain(
                    RetiredRemoteOwner {
                        session,
                        allocation,
                        thread: None,
                    },
                    format!("remote allocation release failed: {error}"),
                ),
            }
        } else if let Some(mut session) = self.session.take() {
            session.close();
        }
        self.source_address = 0;
        self.destination_address = 0;
    }

    /// `NativeBatchOracle.remote_call_pending`.
    pub fn remote_call_pending(&self) -> bool {
        self.retirement.snapshot().pending_remote_call
    }

    /// A durable receipt that remains valid after this oracle is dropped.
    pub fn retirement(&self) -> OracleRetirement {
        self.retirement.clone()
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    fn require_open(&self) -> Result<(), RuntimeError> {
        if self.session.is_none() || self.allocation.is_none() {
            return Err(RuntimeError::SessionNotOpen);
        }
        Ok(())
    }

    fn canonicalize_address(&self) -> u64 {
        self.module_base + self.profile.canonicalize.rva
    }

    fn finalize_effect_address(&self) -> u64 {
        self.module_base + self.profile.finalize_effect.rva
    }

    /// `NativeBatchOracle._retire_inflight_allocation`: hand the allocation to
    /// a waiter that frees it only after the remote thread exits.
    fn retire_inflight(&mut self, thread: u64) -> Result<(), RuntimeError> {
        self.retire_inflight_with(thread, |receiver, retirement| {
            std::thread::Builder::new()
                .name("nioh3-retired-native-call".to_string())
                .spawn(move || {
                    if let Ok(owner) = receiver.recv() {
                        finish_retired_owner(owner, retirement);
                    }
                })
                .map(|_| ())
        })
    }

    fn retire_inflight_with<F>(&mut self, thread: u64, spawn: F) -> Result<(), RuntimeError>
    where
        F: FnOnce(mpsc::Receiver<RetiredRemoteOwner>, OracleRetirement) -> std::io::Result<()>,
    {
        if self.session.is_none() {
            return Err(RuntimeError::OracleRejected {
                detail: "cannot retire an incomplete native call allocation".to_string(),
            });
        }
        let Some(allocation) = self.allocation else {
            return Err(RuntimeError::OracleRejected {
                detail: "cannot retire an incomplete native call allocation".to_string(),
            });
        };
        let (sender, receiver) = mpsc::sync_channel(1);
        if let Err(error) = spawn(receiver, self.retirement.clone()) {
            let session = self.session.take().ok_or(RuntimeError::SessionNotOpen)?;
            self.allocation = None;
            self.source_address = 0;
            self.destination_address = 0;
            self.retirement.retain(
                RetiredRemoteOwner {
                    session,
                    allocation,
                    thread: Some(thread),
                },
                format!("native-call reaper spawn failed: {error}"),
            );
            return Err(RuntimeError::OracleRejected {
                detail: "cannot retire an incomplete native call allocation".to_string(),
            });
        }
        let session = self.session.take().ok_or(RuntimeError::SessionNotOpen)?;
        self.allocation = None;
        self.source_address = 0;
        self.destination_address = 0;
        self.retirement.mark_reaper_pending(allocation, thread);
        if let Err(error) = sender.send(RetiredRemoteOwner {
            session,
            allocation,
            thread: Some(thread),
        }) {
            self.retirement.retain(
                error.0,
                "native-call reaper stopped before ownership transfer".to_string(),
            );
            return Err(RuntimeError::OracleRejected {
                detail: "cannot transfer an incomplete native call allocation".to_string(),
            });
        }
        Ok(())
    }

    /// The one remote-call shape every shipped wrapper uses.
    fn call_with(
        &mut self,
        wrapper: Vec<u8>,
        source_payload: &[u8],
        output_size: usize,
        timeout_ms: u32,
    ) -> Result<Vec<u8>, RuntimeError> {
        self.require_open()?;
        if wrapper.len() as u64 > REMOTE_CODE_SIZE {
            return Err(RuntimeError::OracleRejected {
                detail: "remote wrapper exceeds the code region".to_string(),
            });
        }
        let allocation = self.allocation.unwrap_or_default();
        let session = self.session.as_mut().ok_or(RuntimeError::SessionNotOpen)?;
        session.write(self.source_address, source_payload)?;
        session.write(self.destination_address, &vec![0u8; output_size])?;
        session.write(allocation, &wrapper)?;
        let thread = session.create_remote_thread(allocation)?;
        let waited = session.wait_thread(thread, timeout_ms)?;
        if waited != WAIT_OBJECT_0 {
            self.retire_inflight(thread)?;
            return Err(RuntimeError::OracleRejected {
                detail: format!("等待游戏原生生成器失败：{waited:#x}"),
            });
        }
        let exit_code = session.thread_exit_code(thread)?;
        session.close_thread(thread);
        if exit_code != 0 {
            return Err(RuntimeError::OracleRejected {
                detail: format!("游戏原生生成器线程返回异常：{exit_code:#x}"),
            });
        }
        let output = session.read(self.destination_address, output_size)?;
        Ok(output)
    }

    fn split_records(output: &[u8], expected: usize) -> Result<Vec<Vec<u8>>, RuntimeError> {
        if output.len() != expected * SCROLL_RECORD_SIZE {
            return Err(RuntimeError::OracleRejected {
                detail: "game returned the wrong number of records".to_string(),
            });
        }
        let mut records = Vec::with_capacity(expected);
        for index in 0..expected {
            let start = index * SCROLL_RECORD_SIZE;
            let record = output[start..start + SCROLL_RECORD_SIZE].to_vec();
            if u16::from_le_bytes([record[0], record[1]]) == 0 {
                return Err(RuntimeError::OracleRejected {
                    detail: "游戏原生生成器返回了空记录".to_string(),
                });
            }
            records.push(record);
        }
        Ok(records)
    }

    /// `NativeBatchOracle._preserve_rarity_headers`.
    fn preserve_rarity_headers(
        &self,
        records: Vec<Vec<u8>>,
        requested: &[u8],
    ) -> Result<Vec<Vec<u8>>, RuntimeError> {
        if !self.preserve_requested_rarity || self.profile.display_version != "PC v2.01" {
            return Ok(records);
        }
        let mut preserved = Vec::with_capacity(records.len());
        for (record, request) in records.into_iter().zip(requested.iter()) {
            if *request != 5 || record[0x30..0x32] == [0x05, 0x05] {
                preserved.push(record);
                continue;
            }
            if record[0x30..0x32] != [0x04, 0x04] {
                return Err(RuntimeError::OracleRejected {
                    detail: "PC v2.01 returned an unexpected rarity header for a raw rarity-5 generation request".to_string(),
                });
            }
            let mut adjusted = record;
            adjusted[0x30..0x32].copy_from_slice(&[0x05, 0x05]);
            preserved.push(adjusted);
        }
        Ok(preserved)
    }

    /// `NativeBatchOracle.generate`.
    pub fn generate(
        &mut self,
        source_records: &[Vec<u8>],
        timeout_ms: u32,
    ) -> Result<Vec<Vec<u8>>, RuntimeError> {
        if source_records.is_empty() || source_records.len() > self.max_batch_size {
            return Err(RuntimeError::OracleRejected {
                detail: "source_records must fit in the configured batch".to_string(),
            });
        }
        if source_records
            .iter()
            .any(|record| record.len() != SCROLL_RECORD_SIZE)
        {
            return Err(RuntimeError::OracleRejected {
                detail: "every source record must be exactly 0xE8 bytes".to_string(),
            });
        }
        let count = source_records.len() as u32;
        let payload: Vec<u8> = source_records.iter().flatten().copied().collect();
        let wrapper = build_batch_wrapper(
            self.source_address,
            self.destination_address,
            self.canonicalize_address(),
            count,
        )?;
        let output = self.call_with(
            wrapper,
            &payload,
            source_records.len() * SCROLL_RECORD_SIZE,
            timeout_ms,
        )?;
        let records = Self::split_records(&output, source_records.len())?;
        let requested: Vec<u8> = source_records.iter().map(|record| record[0x31]).collect();
        self.preserve_rarity_headers(records, &requested)
    }

    /// `NativeBatchOracle.generate_seed_range`.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_seed_range(
        &mut self,
        template: &[u8],
        start_seed: u32,
        seed_step: u32,
        count: u32,
        playthrough: Option<u32>,
        generation_mode: u32,
        timeout_ms: u32,
    ) -> Result<Vec<Vec<u8>>, RuntimeError> {
        self.require_open()?;
        if template.len() != SCROLL_RECORD_SIZE {
            return Err(RuntimeError::OracleRejected {
                detail: "template must be exactly 0xE8 bytes".to_string(),
            });
        }
        if count == 0 || count as usize > self.max_batch_size {
            return Err(RuntimeError::OracleRejected {
                detail: "count must fit in the configured batch".to_string(),
            });
        }
        if let Some(playthrough) = playthrough {
            if !(1..=5).contains(&playthrough) {
                return Err(RuntimeError::OracleRejected {
                    detail: "playthrough must be between 1 and 5, or None".to_string(),
                });
            }
        }
        if generation_mode > 1 {
            return Err(RuntimeError::OracleRejected {
                detail: "generation_mode must be 0 or 1".to_string(),
            });
        }
        if playthrough.is_none() && generation_mode != 0 {
            return Err(RuntimeError::OracleRejected {
                detail: "generation_mode 1 requires an explicit playthrough context".to_string(),
            });
        }
        let wrapper = match playthrough {
            None => build_seed_range_wrapper(
                self.source_address,
                self.destination_address,
                self.canonicalize_address(),
                start_seed,
                seed_step,
                count,
            )?,
            Some(playthrough) => {
                let chain = PlaythroughChainRvas::from_profile(&self.profile)?;
                let session = self.session.as_mut().ok_or(RuntimeError::SessionNotOpen)?;
                let manager_pointer = read_u64(
                    &mut **session,
                    self.module_base + chain.playthrough_manager_pointer,
                )?;
                if manager_pointer == 0 {
                    return Err(RuntimeError::OracleRejected {
                        detail: "游戏周目管理器尚未初始化".to_string(),
                    });
                }
                let table = read_u64(&mut **session, manager_pointer + 8)?;
                if table == 0 {
                    return Err(RuntimeError::OracleRejected {
                        detail: "游戏周目参数表尚未初始化".to_string(),
                    });
                }
                build_explicit_playthrough_seed_range_wrapper(
                    self.source_address,
                    self.destination_address,
                    self.module_base,
                    start_seed,
                    seed_step,
                    count,
                    playthrough,
                    generation_mode,
                    &chain,
                )?
            }
        };
        let output = self.call_with(
            wrapper,
            template,
            count as usize * SCROLL_RECORD_SIZE,
            timeout_ms,
        )?;
        let records = Self::split_records(&output, count as usize)?;
        let request = template[0x31];
        self.preserve_rarity_headers(records, &vec![request; records_capacity(&output)])
    }

    /// `NativeBatchOracle.finalize_effect_stage`.
    pub fn finalize_effect_stage(
        &mut self,
        source_record: &[u8],
        effect_index: u32,
        reveal: bool,
        timeout_ms: u32,
    ) -> Result<Vec<u8>, RuntimeError> {
        self.require_open()?;
        if source_record.len() != SCROLL_RECORD_SIZE {
            return Err(RuntimeError::OracleRejected {
                detail: "source_record must be exactly 0xE8 bytes".to_string(),
            });
        }
        if effect_index >= 7 {
            return Err(RuntimeError::OracleRejected {
                detail: "effect_index must be between 0 and 6".to_string(),
            });
        }
        let wrapper = build_effect_finalizer_wrapper(
            self.source_address,
            self.destination_address,
            self.finalize_effect_address(),
            effect_index,
            reveal,
        )?;
        let output = self.call_with(wrapper, source_record, SCROLL_RECORD_SIZE, timeout_ms)?;
        let records = Self::split_records(&output, 1)?;
        records
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::OracleRejected {
                detail: "游戏原生最终化函数返回了空记录".to_string(),
            })
    }

    /// `NativeBatchOracle.finalize_effect_stage_batch`.
    pub fn finalize_effect_stage_batch(
        &mut self,
        source_records: &[Vec<u8>],
        effect_index: u32,
        reveal: bool,
        timeout_ms: u32,
    ) -> Result<Vec<Vec<u8>>, RuntimeError> {
        self.require_open()?;
        if source_records.is_empty() || source_records.len() > self.max_batch_size {
            return Err(RuntimeError::OracleRejected {
                detail: "source_records must fit in the configured batch".to_string(),
            });
        }
        if effect_index >= 7 {
            return Err(RuntimeError::OracleRejected {
                detail: "effect_index must be between 0 and 6".to_string(),
            });
        }
        let payload: Vec<u8> = source_records.iter().flatten().copied().collect();
        let wrapper = build_effect_finalizer_batch_wrapper(
            self.source_address,
            self.destination_address,
            self.finalize_effect_address(),
            source_records.len() as u32,
            effect_index,
            reveal,
        )?;
        let output = self.call_with(
            wrapper,
            &payload,
            source_records.len() * SCROLL_RECORD_SIZE,
            timeout_ms,
        )?;
        Self::split_records(&output, source_records.len())
    }

    /// `NativeBatchOracle.finalize_stage_record`.
    pub fn finalize_stage_record(
        &mut self,
        source_record: &[u8],
        reveal: bool,
        timeout_ms: u32,
    ) -> Result<(Vec<u8>, Option<usize>), RuntimeError> {
        if source_record.len() != SCROLL_RECORD_SIZE {
            return Err(RuntimeError::OracleRejected {
                detail: "source_record must be exactly 0xE8 bytes".to_string(),
            });
        }
        for index in 0..7usize {
            let word = u16::from_le_bytes([
                source_record[effect_word_offset(index)],
                source_record[effect_word_offset(index) + 1],
            ]);
            if word == 0 {
                continue;
            }
            if source_record[effect_word_offset(index)] & 0x40 != 0
                || source_record[effect_flag_offset(index)] & 0x04 != 0
            {
                continue;
            }
            let candidate =
                self.finalize_effect_stage(source_record, index as u32, reveal, timeout_ms)?;
            if candidate[effect_flag_offset(index)] & 0x04 != 0 {
                return Ok((candidate, Some(index)));
            }
        }
        Ok((source_record.to_vec(), None))
    }

    /// `NativeBatchOracle.finalize_stage_records_batch`.
    pub fn finalize_stage_records_batch(
        &mut self,
        source_records: &[Vec<u8>],
        reveal: bool,
        timeout_ms: u32,
    ) -> Result<Vec<Vec<u8>>, RuntimeError> {
        if source_records.is_empty() || source_records.len() > self.max_batch_size {
            return Err(RuntimeError::OracleRejected {
                detail: "source_records must fit in the configured batch".to_string(),
            });
        }
        let mut completed: Vec<Vec<u8>> = source_records.to_vec();
        let mut pending: Vec<usize> = (0..source_records.len()).collect();
        for index in 0..7usize {
            let eligible: Vec<usize> = pending
                .iter()
                .copied()
                .filter(|candidate| {
                    let record = &source_records[*candidate];
                    let word = u16::from_le_bytes([
                        record[effect_word_offset(index)],
                        record[effect_word_offset(index) + 1],
                    ]);
                    word != 0
                        && record[effect_word_offset(index)] & 0x40 == 0
                        && record[effect_flag_offset(index)] & 0x04 == 0
                })
                .collect();
            if eligible.is_empty() {
                continue;
            }
            let batch: Vec<Vec<u8>> = eligible
                .iter()
                .map(|candidate| source_records[*candidate].clone())
                .collect();
            let outputs =
                self.finalize_effect_stage_batch(&batch, index as u32, reveal, timeout_ms)?;
            if outputs.len() != eligible.len() {
                return Err(RuntimeError::OracleRejected {
                    detail: "游戏原生批量最终化函数返回了错误数量的记录".to_string(),
                });
            }
            for (candidate, output) in eligible.iter().zip(outputs.into_iter()) {
                if output[effect_flag_offset(index)] & 0x04 != 0 {
                    completed[*candidate] = output;
                    pending.retain(|value| value != candidate);
                }
            }
        }
        Ok(completed)
    }
}

fn records_capacity(output: &[u8]) -> usize {
    output.len() / SCROLL_RECORD_SIZE
}

fn read_u64(session: &mut dyn RemoteSession, address: u64) -> Result<u64, RuntimeError> {
    let raw = session.read(address, 8)?;
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&raw);
    Ok(u64::from_le_bytes(bytes))
}

/// The bytes `build_source_record` produces, re-exported for the host loops that
/// turn a search request into an oracle batch.
pub fn source_record(
    template: &[u8],
    seed: u32,
    rarity: u8,
    level: u16,
    recommended_level: u16,
    transfer_count: u32,
) -> Result<Vec<u8>, RuntimeError> {
    crate::mutation::native_abi::build_source_record(
        template,
        seed,
        rarity,
        level,
        recommended_level,
        transfer_count,
    )
}

/// Lowercase hex for the receipts and reports that carry oracle output.
pub fn records_hex(records: &[Vec<u8>]) -> String {
    hex(&records.concat())
}

#[cfg(test)]
mod retirement_tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use crate::profile::ProfileSite;
    use std::time::Duration;

    #[derive(Default)]
    struct Controls {
        completed: bool,
        fail_free: bool,
        free_calls: usize,
        closed: bool,
    }

    struct FakeRemoteSession {
        controls: Arc<Mutex<Controls>>,
    }

    impl RemoteSession for FakeRemoteSession {
        fn pid(&self) -> u32 {
            7
        }

        fn read(&mut self, _address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
            Ok(vec![0; size])
        }

        fn write(&mut self, _address: u64, _data: &[u8]) -> Result<(), RuntimeError> {
            Ok(())
        }

        fn allocate(&mut self, _size: usize) -> Result<u64, RuntimeError> {
            Ok(0x5000)
        }

        fn free(&mut self, address: u64) -> Result<(), RuntimeError> {
            let mut controls = self.controls.lock().unwrap();
            controls.free_calls += 1;
            if controls.fail_free {
                Err(RuntimeError::AllocationRelease { address, code: 5 })
            } else {
                Ok(())
            }
        }

        fn create_remote_thread(&mut self, _start: u64) -> Result<u64, RuntimeError> {
            Ok(0x7000)
        }

        fn wait_thread(&mut self, _thread: u64, _milliseconds: u32) -> Result<u32, RuntimeError> {
            Ok(if self.controls.lock().unwrap().completed {
                WAIT_OBJECT_0
            } else {
                WAIT_TIMEOUT
            })
        }

        fn thread_exit_code(&mut self, _thread: u64) -> Result<u32, RuntimeError> {
            Ok(0)
        }

        fn close_thread(&mut self, _thread: u64) {}

        fn close(&mut self) {
            self.controls.lock().unwrap().closed = true;
        }
    }

    fn profile() -> NativeRuntimeProfile {
        let site = |name| ProfileSite {
            name,
            rva: 1,
            signature: vec![0x90],
        };
        NativeRuntimeProfile {
            display_version: "test".to_string(),
            canonicalize: site("canonicalize"),
            finalize_effect: site("finalize_effect"),
            descriptor_complete: site("descriptor_complete"),
            native_signatures: Vec::new(),
            playthrough_selector_pointer_rva: 0,
        }
    }

    fn owned_oracle(controls: Arc<Mutex<Controls>>) -> NativeBatchOracle {
        let mut oracle = NativeBatchOracle::new(7, 0x1000, profile(), 1, false).unwrap();
        oracle.session = Some(Box::new(FakeRemoteSession { controls }));
        oracle.allocation = Some(0x5000);
        oracle.source_address = 0x6000;
        oracle.destination_address = 0x6100;
        oracle
    }

    #[test]
    fn reaper_spawn_failure_keeps_the_original_owner_and_allocation() {
        let controls = Arc::new(Mutex::new(Controls::default()));
        let mut oracle = owned_oracle(Arc::clone(&controls));
        let result = oracle.retire_inflight_with(0x7000, |_receiver, _retirement| {
            Err(std::io::Error::other("injected spawn failure"))
        });
        assert!(result.is_err());
        let snapshot = oracle.retirement().snapshot();
        assert_eq!(snapshot.lifecycle, "retained_unsafe");
        assert_eq!(snapshot.allocation, Some(0x5000));
        assert_eq!(snapshot.thread, Some(0x7000));
        assert!(snapshot.error.unwrap().contains("spawn failed"));
        assert!(oracle.remote_call_pending());
        assert_eq!(controls.lock().unwrap().free_calls, 0);
    }

    #[test]
    fn late_completion_is_polled_then_released_by_the_retained_owner() {
        let controls = Arc::new(Mutex::new(Controls::default()));
        let mut oracle = owned_oracle(Arc::clone(&controls));
        oracle
            .retire_inflight_with(0x7000, |_receiver, _retirement| {
                Err(std::io::Error::other("injected spawn failure"))
            })
            .unwrap_err();
        let retirement = oracle.retirement();
        assert!(!retirement.refresh());
        assert_eq!(controls.lock().unwrap().free_calls, 0);
        controls.lock().unwrap().completed = true;
        assert!(retirement.refresh());
        assert!(retirement.snapshot().safe_to_shutdown);
        let controls = controls.lock().unwrap();
        assert_eq!(controls.free_calls, 1);
        assert!(controls.closed);
    }

    #[test]
    fn free_failure_retains_address_owner_and_error_until_retry_succeeds() {
        let controls = Arc::new(Mutex::new(Controls {
            completed: true,
            fail_free: true,
            ..Controls::default()
        }));
        let mut oracle = owned_oracle(Arc::clone(&controls));
        let retirement = oracle.retirement();
        oracle.close();
        let snapshot = retirement.snapshot();
        assert_eq!(snapshot.lifecycle, "retained_unsafe");
        assert_eq!(snapshot.allocation, Some(0x5000));
        assert!(snapshot.error.unwrap().contains("release failed"));
        assert!(!snapshot.safe_to_shutdown);
        controls.lock().unwrap().fail_free = false;
        assert!(retirement.refresh());
        assert_eq!(controls.lock().unwrap().free_calls, 2);
    }

    #[test]
    fn live_reaper_owns_until_terminal_free() {
        let controls = Arc::new(Mutex::new(Controls {
            completed: true,
            ..Controls::default()
        }));
        let mut oracle = owned_oracle(Arc::clone(&controls));
        let retirement = oracle.retirement();
        // The reaper signals completion so the assertion never depends on
        // scheduler luck between the spawn and the snapshot.
        let (finished_sender, finished_receiver) = mpsc::channel();
        oracle
            .retire_inflight_with(0x7000, |receiver, receipt| {
                std::thread::Builder::new()
                    .spawn(move || {
                        finish_retired_owner(receiver.recv().unwrap(), receipt);
                        let _ = finished_sender.send(());
                    })
                    .map(|_| ())
            })
            .unwrap();
        let finished = finished_receiver.recv_timeout(Duration::from_secs(10));
        assert!(finished.is_ok(), "the retired reaper did not finish");
        let snapshot = retirement.snapshot();
        assert_eq!(snapshot.lifecycle, "released");
        assert!(snapshot.safe_to_shutdown);
        assert!(!snapshot.pending_remote_call);
        assert_eq!(controls.lock().unwrap().free_calls, 1);
        assert!(controls.lock().unwrap().closed);
    }
}
