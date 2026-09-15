//! Fault-injecting adapter for tests and the cross-language gate.
//!
//! Compiled only for `cfg(test)` or with the off-by-default `test-fake`
//! feature, so the shipped library keeps exactly one process implementation. It
//! exists so the restoration and ownership paths can be exercised at every
//! failure point without a game process.

use super::count::{CountMemory, TargetCapture};
use super::memory::TargetProcess;
use super::session::SessionMemory;
use crate::error::RuntimeError;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Base address the fake reports as the module base.
pub const FAKE_MODULE_BASE: u64 = 0x1_4000_0000;
/// Address the fake hands out for the first executable allocation.
pub const FAKE_ALLOCATION: u64 = 0x1_5000_0000;

/// Every injected failure the tests use.
#[derive(Debug, Clone, Default)]
pub struct Faults {
    /// Fail the nth plain write (1-based).
    pub fail_write_call: Option<usize>,
    /// Fail the nth code write.
    pub fail_write_code_call: Option<usize>,
    /// Fail the nth read.
    pub fail_read_call: Option<usize>,
    /// Fail every read from the nth onward, which is what a target that stops
    /// answering looks like to a session that must keep its ownership.
    pub fail_reads_after: Option<usize>,
    /// Report fewer bytes written than requested.
    pub truncated_write: bool,
    pub allocation_fails: bool,
    pub release_fails: bool,
    /// `false` for `exited()`, which is what an unreachable query must report.
    pub exit_query_fails: bool,
    pub exited: bool,
    /// Drop writes: simulates a target that keeps its own bytes.
    pub quiet_writes: bool,
    /// Readback returned after a successful count write.
    pub readback_after_write: Option<Vec<u8>>,
    /// Creation FILETIME the handle reports.
    pub creation_filetime: u64,
    /// Creation FILETIME the handle reports instead, when set.
    pub creation_filetime_override: Option<u64>,
}

/// Observable state of the fake target.
#[derive(Debug, Default)]
pub struct State {
    pub memory: BTreeMap<u64, Vec<u8>>,
    pub writes: Vec<(u64, Vec<u8>)>,
    pub code_writes: Vec<(u64, Vec<u8>)>,
    pub read_calls: usize,
    /// Plain writes attempted (trampoline buffer, counter slot).
    pub write_calls: usize,
    /// Executable writes attempted (the hook patch).
    pub code_write_calls: usize,
    pub allocated: Vec<u64>,
    pub freed: Vec<u64>,
    pub closed: usize,
    pub next_allocation: u64,
}

impl State {
    pub fn seed(&mut self, address: u64, data: &[u8]) {
        self.memory.insert(address, data.to_vec());
    }

    pub fn bytes(&self, address: u64, size: usize) -> Vec<u8> {
        self.memory
            .get(&address)
            .map(|data| data[..size.min(data.len())].to_vec())
            .unwrap_or_default()
    }
}

/// Opens fake processes that all share one [`State`].
pub struct FakeMemoryFactory {
    pub state: Rc<RefCell<State>>,
    pub faults: Faults,
}

impl FakeMemoryFactory {
    pub fn new() -> Self {
        Self::with_faults(Faults::default())
    }

    pub fn with_faults(faults: Faults) -> Self {
        Self {
            state: Rc::new(RefCell::new(State {
                next_allocation: FAKE_ALLOCATION,
                ..State::default()
            })),
            faults,
        }
    }
}

impl Default for FakeMemoryFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionMemory for FakeMemoryFactory {
    type Process = FakeProcess;

    fn module_base(&self, _pid: u32, _module_name: &str) -> Result<u64, RuntimeError> {
        Ok(FAKE_MODULE_BASE)
    }

    fn creation_filetime(&self, _pid: u32) -> Result<Option<u64>, RuntimeError> {
        // The value resolved before any handle exists; the handle may report a
        // different one, which is exactly how a recycled pid is detected.
        Ok(Some(self.faults.creation_filetime))
    }

    fn open(&self, pid: u32) -> Result<FakeProcess, RuntimeError> {
        Ok(FakeProcess {
            state: Rc::clone(&self.state),
            faults: self.faults.clone(),
            pid,
        })
    }
}

/// One fake target process.
pub struct FakeProcess {
    state: Rc<RefCell<State>>,
    faults: Faults,
    pid: u32,
}

impl FakeProcess {
    fn next_read_fails(&self, state: &State) -> bool {
        self.faults
            .fail_read_call
            .is_some_and(|index| state.read_calls == index)
            || self
                .faults
                .fail_reads_after
                .is_some_and(|index| state.read_calls >= index)
    }

    fn write_inner(&self, address: u64, data: &[u8], code: bool) -> Result<(), RuntimeError> {
        let mut state = self.state.borrow_mut();
        let (failure, calls) = if code {
            state.code_write_calls += 1;
            (self.faults.fail_write_code_call, state.code_write_calls)
        } else {
            state.write_calls += 1;
            (self.faults.fail_write_call, state.write_calls)
        };
        if failure.is_some_and(|index| calls == index) {
            return Err(RuntimeError::MemoryWrite {
                address,
                size: data.len(),
                code: 5,
            });
        }
        if self.faults.truncated_write {
            return Err(RuntimeError::ShortWrite {
                address,
                expected: data.len(),
                actual: data.len() - 1,
            });
        }
        if code {
            state.code_writes.push((address, data.to_vec()));
        } else {
            state.writes.push((address, data.to_vec()));
        }
        if !self.faults.quiet_writes {
            state.memory.insert(address, data.to_vec());
        }
        Ok(())
    }
}

impl TargetProcess for FakeProcess {
    fn pid(&self) -> u32 {
        self.pid
    }

    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
        let mut state = self.state.borrow_mut();
        state.read_calls += 1;
        if self.next_read_fails(&state) {
            return Err(RuntimeError::MemoryRead {
                address,
                size,
                code: 6,
            });
        }
        match state.memory.get(&address) {
            Some(data) if data.len() >= size => Ok(data[..size].to_vec()),
            _ => Err(RuntimeError::MemoryRead {
                address,
                size,
                code: 299,
            }),
        }
    }

    fn write(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError> {
        self.write_inner(address, data, false)
    }

    fn write_code(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError> {
        self.write_inner(address, data, true)
    }

    fn allocate_executable_near(&mut self, address: u64, size: usize) -> Result<u64, RuntimeError> {
        if self.faults.allocation_fails {
            return Err(RuntimeError::AllocationUnavailable {
                address,
                size: size as u64,
            });
        }
        let mut state = self.state.borrow_mut();
        let allocation = state.next_allocation;
        state.next_allocation += 0x1000;
        // The real allocator refuses an address outside the rel32 range, so the
        // fake does too.
        super::trampoline::build_relative_jump(address, allocation)?;
        state.allocated.push(allocation);
        state.memory.insert(allocation, vec![0u8; size]);
        Ok(allocation)
    }

    fn free_allocation(&mut self, address: u64) -> Result<(), RuntimeError> {
        if self.faults.release_fails {
            return Err(RuntimeError::AllocationRelease { address, code: 487 });
        }
        let mut state = self.state.borrow_mut();
        state.freed.push(address);
        state.memory.remove(&address);
        Ok(())
    }

    fn exited(&mut self) -> Result<bool, RuntimeError> {
        if self.faults.exit_query_fails {
            return Ok(false);
        }
        Ok(self.faults.exited)
    }

    fn creation_filetime(&mut self) -> Result<Option<u64>, RuntimeError> {
        Ok(Some(
            self.faults
                .creation_filetime_override
                .unwrap_or(self.faults.creation_filetime),
        ))
    }

    fn close(&mut self) {
        self.state.borrow_mut().closed += 1;
    }
}

/// Record-shaped fake of `runtime_count_edit.WindowsCountMemory`.
pub struct FakeCountMemory {
    pub pid: u32,
    pub creation_time: String,
    pub manager: u64,
    pub data: u64,
    pub address: u64,
    pub serial: u64,
    pub record: Vec<u8>,
    pub faults: Faults,
    pub captures: usize,
    pub writes: usize,
}

impl FakeCountMemory {
    /// A record whose defined fields are stable across capture and write.
    pub fn new(record: Vec<u8>) -> Self {
        Self {
            pid: 4321,
            creation_time: "134338049984156850".to_string(),
            manager: 0x1_0000_0000,
            data: 0x2_0000_0000,
            address: 0x2_0000_1000,
            serial: 0x1122_3344_5566_7788,
            record,
            faults: Faults::default(),
            captures: 0,
            writes: 0,
        }
    }

    pub fn with_faults(record: Vec<u8>, faults: Faults) -> Self {
        let mut memory = Self::new(record);
        memory.faults = faults;
        memory
    }

    fn capture_of(&self) -> TargetCapture {
        TargetCapture {
            pid: self.pid,
            creation_time: self.creation_time.clone(),
            manager: self.manager,
            data: self.data,
            address: self.address,
            record_hex: self
                .record
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            serial: self.serial,
        }
    }
}

impl CountMemory for FakeCountMemory {
    fn capture(&mut self, serial: u64) -> Result<TargetCapture, RuntimeError> {
        self.captures += 1;
        if serial != self.serial {
            return Err(RuntimeError::CountInstanceUnavailable { serial });
        }
        Ok(self.capture_of())
    }

    fn write(&mut self, expected: &TargetCapture, desired: u8) -> Result<Vec<u8>, RuntimeError> {
        self.writes += 1;
        if expected != &self.capture_of() {
            return Err(RuntimeError::CountInstanceChanged);
        }
        if self.faults.fail_write_call == Some(self.writes) {
            return Err(RuntimeError::MemoryWrite {
                address: self.address + 0x33,
                size: 1,
                code: 5,
            });
        }
        if !self.faults.quiet_writes {
            self.record[0x33] = desired;
        }
        Ok(match &self.faults.readback_after_write {
            Some(override_record) => override_record.clone(),
            None => self.record.clone(),
        })
    }
}
