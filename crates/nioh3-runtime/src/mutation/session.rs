//! Temporary hook sessions and the ownership they retain.
//!
//! Port of `runtime_auxiliary_override.RuntimeAuxiliaryOverrideSession`,
//! `runtime_challenge_override.RuntimeChallengeOverrideSession` and
//! `runtime_challenge_override.OverrideGroup`, plus the `start_override` /
//! `stop_override` ownership rules of `RuntimeApplication`.
//!
//! Ownership is the point of this module: once a session installs a hook it
//! keeps the target, the allocation and the patch bytes until a restoration is
//! positively confirmed, and it never overwrites bytes that another writer
//! changed underneath it.

use crate::error::RuntimeError;
use crate::mutation::live_add::LiveAddOwnership;
use crate::mutation::memory::TargetProcess;
use crate::mutation::trampoline::{
    build_challenge_trampoline, build_override_trampoline, build_relative_jump, OverrideProfile,
    TRAMPOLINE_CAPACITY,
};
use crate::platform::{module_range, process_creation_filetime, ProcessIdentity, GAME_MODULE_NAME};
use crate::profile::NativeRuntimeProfile;
use crate::status::{OverrideSession as OwnershipState, RuntimeOwnership, RuntimeStatus};

/// `REMOTE_ALLOCATION_SIZE`.
pub const REMOTE_ALLOCATION_SIZE: usize = 0x1000;
/// The counter occupies the last eight bytes of the allocation.
pub const COUNTER_RESERVE: usize = 8;

/// `runtime_challenge_override.CAPACITY_RVA`.
pub const CAPACITY_RVA: u64 = 0x1028E30;
/// The PC v2.02 challenge-capacity getter.
///
/// The function body is byte-identical to the PC v2.01 getter at
/// [`CAPACITY_RVA`] apart from rel32/RIP-relative displacements, and its
/// relocation-free prefix occurs exactly once in the v2.02 `.text`.
pub const PC_V202_CAPACITY_RVA: u64 = 0x102AD60;
/// `runtime_challenge_override.CAPACITY_SIGNATURE`.
pub const CAPACITY_SIGNATURE: [u8; 5] = [0x48, 0x89, 0x5C, 0x24, 0x08];
/// The display version the challenge getter was first verified against.
pub const CHALLENGE_DISPLAY_VERSION: &str = "PC v2.01";

/// The challenge-capacity getter of one verified display version.
pub fn challenge_capacity_rva(display_version: &str) -> Option<u64> {
    match display_version {
        CHALLENGE_DISPLAY_VERSION => Some(CAPACITY_RVA),
        "PC v2.02" => Some(PC_V202_CAPACITY_RVA),
        _ => None,
    }
}

/// How a session obtains the target: module base, pre-write identity, handle.
///
/// Splitting this out of [`TargetProcess`] lets the ownership tests run against
/// a fault-injecting target while the Windows binding stays the only code that
/// opens a real handle.
pub trait SessionMemory {
    type Process: TargetProcess;

    /// Image base of the module the site address is relative to.
    fn module_base(&self, pid: u32, module_name: &str) -> Result<u64, RuntimeError>;

    /// Process creation FILETIME, resolved before any write handle is opened.
    fn creation_filetime(&self, pid: u32) -> Result<Option<u64>, RuntimeError>;

    /// Open the write handle. Called once per explicit `start`.
    fn open(&self, pid: u32) -> Result<Self::Process, RuntimeError>;
}

/// Which verified site a session hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionSite {
    /// `descriptor_complete` of the approved auxiliary profile.
    DescriptorComplete,
    /// The PC v2.01 challenge capacity getter.
    ChallengeCapacity,
}

/// `runtime_challenge_override.ChallengeOverrideProfile`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChallengeOverrideProfile {
    pub seed: u32,
    pub capacity: u8,
}

impl ChallengeOverrideProfile {
    pub fn validate(&self) -> Result<(), RuntimeError> {
        if !(1..=7).contains(&self.capacity) {
            return Err(RuntimeError::InvalidOverrideProfile {
                detail: "Expected a uint32 seed and a capacity from 1 to 7".to_string(),
            });
        }
        Ok(())
    }
}

enum SessionCode {
    Auxiliary(OverrideProfile),
    Challenge(ChallengeOverrideProfile),
}

/// One installed temporary hook.
pub struct OverrideSession<F: SessionMemory> {
    code: SessionCode,
    site: SessionSite,
    pid: u32,
    module_name: String,
    runtime_profile: NativeRuntimeProfile,
    factory: F,
    memory: Option<F::Process>,
    identity: Option<ProcessIdentity>,
    hook_address: u64,
    hook_bytes: Vec<u8>,
    allocation: u64,
    counter_address: u64,
    patch: Option<Vec<u8>>,
    installed_once: bool,
}

impl<F: SessionMemory> OverrideSession<F> {
    /// Auxiliary descriptor override session.
    pub fn auxiliary(
        profile: OverrideProfile,
        pid: u32,
        runtime_profile: NativeRuntimeProfile,
        factory: F,
    ) -> Result<Self, RuntimeError> {
        profile.validate()?;
        Ok(Self::new(
            SessionCode::Auxiliary(profile),
            SessionSite::DescriptorComplete,
            pid,
            GAME_MODULE_NAME.to_string(),
            runtime_profile,
            factory,
        ))
    }

    /// Challenge capacity session for a version with a verified getter.
    pub fn challenge(
        profile: ChallengeOverrideProfile,
        pid: u32,
        runtime_profile: NativeRuntimeProfile,
        factory: F,
    ) -> Result<Self, RuntimeError> {
        profile.validate()?;
        if challenge_capacity_rva(&runtime_profile.display_version).is_none() {
            return Err(RuntimeError::InvalidOverrideProfile {
                detail: "Challenge capacity override requires verified PC v2.01 or PC v2.02"
                    .to_string(),
            });
        }
        Ok(Self::new(
            SessionCode::Challenge(profile),
            SessionSite::ChallengeCapacity,
            pid,
            GAME_MODULE_NAME.to_string(),
            runtime_profile,
            factory,
        ))
    }

    fn new(
        code: SessionCode,
        site: SessionSite,
        pid: u32,
        module_name: String,
        runtime_profile: NativeRuntimeProfile,
        factory: F,
    ) -> Self {
        Self {
            code,
            site,
            pid,
            module_name,
            runtime_profile,
            factory,
            memory: None,
            identity: None,
            hook_address: 0,
            hook_bytes: Vec::new(),
            allocation: 0,
            counter_address: 0,
            patch: None,
            installed_once: false,
        }
    }

    /// Hook against a differently named module.
    ///
    /// The shipped product always hooks `Nioh3.exe`; the owned helper process
    /// proves the real API path against its own image, which has another name.
    pub fn with_module_name(mut self, module_name: impl Into<String>) -> Self {
        self.module_name = module_name.into();
        self
    }

    /// `RuntimeAuxiliaryOverrideSession.active`.
    pub fn active(&self) -> bool {
        self.memory.is_some() && self.allocation != 0 && self.patch.is_some()
    }

    pub fn site(&self) -> SessionSite {
        self.site
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Identity the session verified before it opened the write handle.
    pub fn identity(&self) -> Option<ProcessIdentity> {
        self.identity
    }

    pub fn hook_address(&self) -> u64 {
        self.hook_address
    }

    pub fn allocation(&self) -> u64 {
        self.allocation
    }

    pub fn counter_address(&self) -> u64 {
        self.counter_address
    }

    pub fn patch(&self) -> Option<&[u8]> {
        self.patch.as_deref()
    }

    /// `hit_count`: zero while no hook is installed, otherwise the counter the
    /// trampoline increments.
    pub fn hit_count(&mut self) -> Result<u64, RuntimeError> {
        if !self.active() {
            return Ok(0);
        }
        let counter = self.counter_address;
        let memory = self.memory.as_mut().ok_or(RuntimeError::SessionNotOpen)?;
        let raw = memory.read(counter, 8)?;
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&raw);
        Ok(u64::from_le_bytes(bytes))
    }

    /// Port of `start`. Fails closed before any write, then installs.
    pub fn start(&mut self) -> Result<(), RuntimeError> {
        if self.active() {
            return Ok(());
        }
        let module_base = self.factory.module_base(self.pid, &self.module_name)?;
        let (hook_rva, hook_bytes) = self.site_bytes();
        self.hook_address = module_base + hook_rva;
        self.hook_bytes = hook_bytes;

        let creation = self
            .factory
            .creation_filetime(self.pid)?
            .ok_or(RuntimeError::ProcessGone { pid: self.pid })?;
        let mut process = self.factory.open(self.pid)?;
        let on_handle = process.creation_filetime()?;
        if on_handle != Some(creation) {
            process.close();
            return Err(RuntimeError::ProcessInstanceChanged { pid: self.pid });
        }

        let actual = process.read(self.hook_address, self.hook_bytes.len());
        match actual {
            Ok(bytes) if bytes == self.hook_bytes => {}
            Ok(_) => {
                process.close();
                return Err(RuntimeError::SignatureMismatch {
                    site: self.site_name().to_string(),
                    rva: hook_rva,
                });
            }
            Err(error) => {
                process.close();
                return Err(error);
            }
        }
        self.identity = Some(ProcessIdentity {
            pid: self.pid,
            creation_filetime: creation,
        });
        self.memory = Some(process);

        match self.install() {
            Ok(()) => Ok(()),
            Err(error) => {
                self.rollback_start();
                Err(error)
            }
        }
    }

    fn install(&mut self) -> Result<(), RuntimeError> {
        let hook_bytes = self.hook_bytes.clone();
        let memory = self.memory.as_mut().ok_or(RuntimeError::SessionNotOpen)?;
        let allocation =
            memory.allocate_executable_near(self.hook_address, REMOTE_ALLOCATION_SIZE)?;
        self.allocation = allocation;
        self.counter_address = allocation + (REMOTE_ALLOCATION_SIZE - COUNTER_RESERVE) as u64;

        let code = match &self.code {
            SessionCode::Auxiliary(profile) => build_override_trampoline(
                profile,
                self.hook_address + hook_bytes.len() as u64,
                Some(self.counter_address),
                &hook_bytes,
            )?,
            SessionCode::Challenge(profile) => build_challenge_trampoline(
                profile.seed,
                profile.capacity,
                self.hook_address + hook_bytes.len() as u64,
                self.counter_address,
                &hook_bytes,
            )?,
        };
        if code.len() > TRAMPOLINE_CAPACITY {
            return Err(RuntimeError::TrampolineTooLarge {
                size: code.len() as u64,
                capacity: TRAMPOLINE_CAPACITY as u64,
            });
        }
        self.memory
            .as_mut()
            .ok_or(RuntimeError::SessionNotOpen)?
            .write(allocation, &code)?;
        self.memory
            .as_mut()
            .ok_or(RuntimeError::SessionNotOpen)?
            .write(self.counter_address, &[0u8; COUNTER_RESERVE])?;
        let patch = build_relative_jump(self.hook_address, allocation)?;
        self.patch = Some(patch.clone());
        self.installed_once = true;
        self.write_hook(&patch)
    }

    fn write_hook(&mut self, data: &[u8]) -> Result<(), RuntimeError> {
        let address = self.hook_address;
        let memory = self.memory.as_mut().ok_or(RuntimeError::SessionNotOpen)?;
        memory.write_code(address, data)
    }

    /// Port of `_rollback_start`. A read failure or a foreign hook is left
    /// exactly as it is: ownership is retained, nothing is overwritten.
    fn rollback_start(&mut self) {
        if self.memory.is_some() && self.patch.is_some() {
            let hook_bytes = self.hook_bytes.clone();
            let length = hook_bytes.len();
            let read = self
                .memory
                .as_mut()
                .map(|memory| memory.read(self.hook_address, length));
            match read {
                Some(Ok(current)) => {
                    if Some(current.as_slice()) == self.patch.as_deref() {
                        let _ = self.write_hook(&hook_bytes);
                    } else if current != hook_bytes {
                        // Another writer owns the hook now. Freeing the
                        // trampoline could leave a dangling executable jump.
                        return;
                    }
                }
                _ => return,
            }
        }
        let release_allocation = !self.installed_once;
        self.release_session(release_allocation);
    }

    /// Port of `stop`. Restores only bytes this session owns.
    pub fn stop(&mut self) -> Result<(), RuntimeError> {
        if self.memory.is_none() {
            return Ok(());
        }
        let hook_bytes = self.hook_bytes.clone();
        let read = self
            .memory
            .as_mut()
            .ok_or(RuntimeError::SessionNotOpen)?
            .read(self.hook_address, hook_bytes.len());
        match read {
            Ok(current) => {
                if Some(current.as_slice()) == self.patch.as_deref() {
                    self.write_hook(&hook_bytes)?;
                } else if current == hook_bytes {
                    // Already back to the verified bytes.
                } else {
                    // Never overwrite a hook another program changed, and never
                    // free a trampoline that may still be jumped into.
                    return Err(RuntimeError::HookModified {
                        address: self.hook_address,
                    });
                }
            }
            Err(error) => {
                if !self.process_exited()? {
                    return Err(RuntimeError::HookRestoreUnverified {
                        detail: error.message(),
                    });
                }
                // A confirmed-terminated process releases its own allocation.
            }
        }
        // A trampoline that has ever been live is retired until the target
        // exits: a game thread may still be executing inside it.
        self.release_session(false);
        Ok(())
    }

    fn process_exited(&mut self) -> Result<bool, RuntimeError> {
        match self.memory.as_mut() {
            Some(memory) => memory.exited(),
            None => Ok(true),
        }
    }

    /// Port of `_release_session`.
    fn release_session(&mut self, release_allocation: bool) {
        if release_allocation && self.allocation != 0 {
            if let Some(memory) = self.memory.as_mut() {
                let _ = memory.free_allocation(self.allocation);
            }
        }
        if let Some(memory) = self.memory.as_mut() {
            memory.close();
        }
        self.memory = None;
        self.allocation = 0;
        self.counter_address = 0;
        self.patch = None;
        self.installed_once = false;
    }

    fn site_bytes(&self) -> (u64, Vec<u8>) {
        match self.site {
            SessionSite::DescriptorComplete => (
                self.runtime_profile.descriptor_complete.rva,
                self.runtime_profile.descriptor_complete.signature.clone(),
            ),
            // `challenge` refuses any other version, so the fallback is never
            // reached; it keeps the shipped v2.01 site rather than inventing one.
            SessionSite::ChallengeCapacity => (
                challenge_capacity_rva(&self.runtime_profile.display_version)
                    .unwrap_or(CAPACITY_RVA),
                CAPACITY_SIGNATURE.to_vec(),
            ),
        }
    }

    fn site_name(&self) -> &'static str {
        match self.site {
            SessionSite::DescriptorComplete => "descriptor_complete",
            SessionSite::ChallengeCapacity => "capacity",
        }
    }
}

/// Port of `runtime_challenge_override.OverrideGroup`.
pub struct OverrideGroup<F: SessionMemory> {
    sessions: Vec<OverrideSession<F>>,
}

impl<F: SessionMemory> OverrideGroup<F> {
    pub fn new(sessions: Vec<OverrideSession<F>>) -> Self {
        Self { sessions }
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub fn sessions(&self) -> &[OverrideSession<F>] {
        &self.sessions
    }

    pub fn start(&mut self) -> Result<(), RuntimeError> {
        for session in self.sessions.iter_mut() {
            session.start()?;
        }
        Ok(())
    }

    /// Every owner is stopped, newest first; the last failure is reported.
    pub fn stop(&mut self) -> Result<(), RuntimeError> {
        let mut last_error = None;
        for session in self.sessions.iter_mut().rev() {
            if let Err(error) = session.stop() {
                last_error = Some(error);
            }
        }
        match last_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub fn hit_count(&mut self) -> Result<u64, RuntimeError> {
        let mut total = 0u64;
        for session in self.sessions.iter_mut() {
            total += session.hit_count()?;
        }
        Ok(total)
    }
}

/// Port of the `RuntimeApplication` override ownership around the group.
pub struct RuntimeMutationHost<F: SessionMemory> {
    ownership: RuntimeOwnership,
    group: Option<OverrideGroup<F>>,
}

impl<F: SessionMemory> Default for RuntimeMutationHost<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: SessionMemory> RuntimeMutationHost<F> {
    pub fn new() -> Self {
        Self {
            ownership: RuntimeOwnership::new(),
            group: None,
        }
    }

    /// Retired-oracle and live-add ownership lives beside the override.
    pub fn ownership_mut(&mut self) -> &mut RuntimeOwnership {
        &mut self.ownership
    }

    pub fn safe_to_shutdown(&mut self) -> bool {
        self.status().safe_to_shutdown
    }

    /// Port of `RuntimeApplication.status` for the override half.
    pub fn status(&mut self) -> RuntimeStatus {
        let state = match self.group.as_mut() {
            None => OwnershipState::Absent,
            Some(group) => match group.hit_count() {
                Ok(0) => OwnershipState::Active { hit_count: 0 },
                Ok(hits) => OwnershipState::Active { hit_count: hits },
                Err(error) => OwnershipState::Faulted {
                    message: error.message(),
                },
            },
        };
        self.ownership.set_session(state);
        self.ownership.status()
    }

    /// Port of `RuntimeApplication.start_override`.
    ///
    /// Ownership is retained before the first hook is written and kept whenever
    /// a rollback cannot be confirmed, so an ambiguous target is never
    /// presented as clean.
    pub fn start_override(
        &mut self,
        sessions: Vec<OverrideSession<F>>,
    ) -> Result<RuntimeStatus, RuntimeError> {
        if !self.safe_to_shutdown() {
            return Err(RuntimeError::RuntimeBusy);
        }
        if sessions.is_empty() {
            return Err(RuntimeError::InvalidOverrideProfile {
                detail: "Select at least one temporary field".to_string(),
            });
        }
        let mut group = OverrideGroup::new(sessions);
        if let Err(error) = group.start() {
            if group.stop().is_err() {
                // Restoration is unconfirmed: keep the owner.
                self.group = Some(group);
                return Err(error);
            }
            return Err(error);
        }
        self.group = Some(group);
        Ok(self.status())
    }

    /// Port of `RuntimeApplication.stop_override`.
    pub fn stop_override(&mut self) -> Result<RuntimeStatus, RuntimeError> {
        if let Some(group) = self.group.as_mut() {
            group.stop()?;
            self.group = None;
        }
        Ok(self.status())
    }

    /// Port of the live-add half of `RuntimeApplication.status`: publish the
    /// live-addition ownership before the status snapshot is taken.
    pub fn set_live_add_ownership(&mut self, ownership: &LiveAddOwnership) {
        self.ownership
            .set_live_add_unsafe(ownership.unsafe_ownership());
    }

    /// `runtime.status` with the live-addition ownership already published.
    pub fn status_with_live_add(&mut self, ownership: &LiveAddOwnership) -> RuntimeStatus {
        self.set_live_add_ownership(ownership);
        self.status()
    }
}

/// Windows session memory: read-only module/identity probes plus one write
/// handle opened per explicit start.
#[cfg(windows)]
pub struct WindowsSessionMemory;

#[cfg(windows)]
impl SessionMemory for WindowsSessionMemory {
    type Process = crate::mutation::memory::WindowsProcess;

    fn module_base(&self, pid: u32, module_name: &str) -> Result<u64, RuntimeError> {
        Ok(module_range(pid, module_name)?.base)
    }

    fn creation_filetime(&self, pid: u32) -> Result<Option<u64>, RuntimeError> {
        process_creation_filetime(pid)
    }

    fn open(&self, pid: u32) -> Result<Self::Process, RuntimeError> {
        crate::mutation::memory::WindowsProcess::open_override(pid)
    }
}

/// The concrete Windows session and host types the protected worker builds on.
#[cfg(windows)]
pub type WindowsOverrideSession = OverrideSession<WindowsSessionMemory>;
#[cfg(windows)]
pub type WindowsMutationHost = RuntimeMutationHost<WindowsSessionMemory>;
