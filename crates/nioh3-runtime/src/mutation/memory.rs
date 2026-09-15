//! Process access for mutation, separated by right level.

use crate::error::RuntimeError;

/// `PROCESS_VM_OPERATION | PROCESS_VM_READ | PROCESS_VM_WRITE |
/// PROCESS_QUERY_INFORMATION`.
///
/// Port of `runtime_auxiliary_override.PROCESS_ACCESS`: one handle for one
/// installed hook, which both reads back the patch and restores it.
pub const OVERRIDE_ACCESS: u32 = 0x0008 | 0x0010 | 0x0020 | 0x0400;

/// `PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_OPERATION | PROCESS_VM_WRITE`.
///
/// Port of the short-lived handle `runtime_count_edit.WindowsCountMemory.write`
/// opens after every planning gate has passed. It deliberately drops
/// `PROCESS_VM_READ`, because the readback goes through the record window that
/// already holds a read view.
pub const COUNT_WRITE_ACCESS: u32 = 0x1000 | 0x0008 | 0x0020;

/// `PROCESS_VM_READ | PROCESS_QUERY_INFORMATION`.
///
/// Port of `process_memory_readonly.ProcessReader`: the view a count edit uses
/// to locate and read back the record before and after the explicit write.
pub const READ_ACCESS: u32 = 0x0010 | 0x0400;

/// The process operations a mutation session needs.
///
/// Deliberately small: no thread creation, termination, suspension or debugger
/// entry point exists here, so no implementation can acquire one through it.
pub trait TargetProcess {
    fn pid(&self) -> u32;

    /// Read bytes through the handle the session already owns.
    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError>;

    /// Write plain data through the handle the session already owns.
    fn write(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError>;

    /// Write executable bytes: raise the page protection, write, flush the
    /// instruction cache, then restore the previous protection.
    fn write_code(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError>;

    /// Allocate `size` committed executable bytes within rel32 range of
    /// `address`, or fail closed.
    fn allocate_executable_near(&mut self, address: u64, size: usize) -> Result<u64, RuntimeError>;

    fn free_allocation(&mut self, address: u64) -> Result<(), RuntimeError>;

    /// `Ok(true)` only when the operating system positively reports that the
    /// target has terminated; a failed query is `Ok(false)`, never "exited".
    fn exited(&mut self) -> Result<bool, RuntimeError>;

    /// Creation FILETIME read from the handle the session owns.
    ///
    /// Opening a handle pins the process object, so comparing this against the
    /// creation time resolved before the open is what makes a recycled pid fail
    /// closed instead of writing into a different process.
    fn creation_filetime(&mut self) -> Result<Option<u64>, RuntimeError>;

    /// Release the handle. Restoration state stays the caller's responsibility.
    fn close(&mut self);
}

/// Real Windows implementation of [`TargetProcess`].
#[cfg(windows)]
pub struct WindowsProcess {
    pid: u32,
    access: u32,
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
mod windows_impl {
    use super::{TargetProcess, WindowsProcess, COUNT_WRITE_ACCESS, OVERRIDE_ACCESS};
    use crate::error::RuntimeError;
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, FILETIME, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::{
        FlushInstructionCache, ReadProcessMemory, WriteProcessMemory,
    };
    use windows_sys::Win32::System::Memory::{
        VirtualAllocEx, VirtualFreeEx, VirtualProtectEx, VirtualQueryEx, MEMORY_BASIC_INFORMATION,
        MEM_COMMIT, MEM_FREE, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
    };
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, GetProcessTimes, OpenProcess};

    const STILL_ACTIVE: u32 = 259;
    const ALLOCATION_GRANULARITY: u64 = 0x10000;
    /// A trampoline must land within `address +- 0x7FFF0000` for its `rel32`.
    const REL32_REACH: u64 = 0x7FFF_0000;

    fn last_error() -> u32 {
        unsafe { GetLastError() }
    }

    fn align_up(value: u64, alignment: u64) -> u64 {
        (value + alignment - 1) & !(alignment - 1)
    }

    impl WindowsProcess {
        /// Open the override handle. Write rights exist only inside this call.
        pub fn open_override(pid: u32) -> Result<Self, RuntimeError> {
            Self::open_with(pid, OVERRIDE_ACCESS)
        }

        /// Open the minimal count-write handle.
        pub fn open_count_write(pid: u32) -> Result<Self, RuntimeError> {
            Self::open_with(pid, COUNT_WRITE_ACCESS)
        }

        /// Open the read view used to locate and read back a record.
        pub fn open_read(pid: u32) -> Result<Self, RuntimeError> {
            Self::open_with(pid, super::READ_ACCESS)
        }

        fn open_with(pid: u32, access: u32) -> Result<Self, RuntimeError> {
            let handle = unsafe { OpenProcess(access, 0, pid) };
            if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                return Err(RuntimeError::OpenProcess {
                    pid,
                    code: last_error(),
                });
            }
            Ok(Self {
                pid,
                access,
                handle,
            })
        }

        /// The access mask this handle actually holds, so a caller can audit it.
        pub fn access(&self) -> u32 {
            self.access
        }

        fn read_raw(&self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
            let mut buffer = vec![0u8; size];
            let mut read: usize = 0;
            let ok = unsafe {
                ReadProcessMemory(
                    self.handle,
                    address as *const c_void,
                    buffer.as_mut_ptr() as *mut c_void,
                    size,
                    &mut read,
                )
            };
            if ok == 0 {
                return Err(RuntimeError::MemoryRead {
                    address,
                    size,
                    code: last_error(),
                });
            }
            if read != size {
                return Err(RuntimeError::ShortRead {
                    address,
                    expected: size,
                    actual: read,
                });
            }
            Ok(buffer)
        }

        fn write_raw(&self, address: u64, data: &[u8]) -> Result<(), RuntimeError> {
            let mut written: usize = 0;
            let ok = unsafe {
                WriteProcessMemory(
                    self.handle,
                    address as *mut c_void,
                    data.as_ptr() as *const c_void,
                    data.len(),
                    &mut written,
                )
            };
            if ok == 0 {
                return Err(RuntimeError::MemoryWrite {
                    address,
                    size: data.len(),
                    code: last_error(),
                });
            }
            if written != data.len() {
                return Err(RuntimeError::ShortWrite {
                    address,
                    expected: data.len(),
                    actual: written,
                });
            }
            Ok(())
        }
    }

    impl Drop for WindowsProcess {
        fn drop(&mut self) {
            self.close();
        }
    }

    unsafe impl Send for WindowsProcess {}

    impl TargetProcess for WindowsProcess {
        fn pid(&self) -> u32 {
            self.pid
        }

        fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
            self.read_raw(address, size)
        }

        fn write(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError> {
            self.write_raw(address, data)
        }

        fn write_code(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError> {
            // Port of `RuntimeAuxiliaryOverrideSession._write_hook`.
            let mut previous: u32 = 0;
            let protected = unsafe {
                VirtualProtectEx(
                    self.handle,
                    address as *const c_void,
                    data.len(),
                    PAGE_EXECUTE_READWRITE,
                    &mut previous,
                )
            };
            if protected == 0 {
                return Err(RuntimeError::MemoryProtect {
                    address,
                    size: data.len(),
                    code: last_error(),
                });
            }
            let result = self.write_raw(address, data).and_then(|()| {
                let flushed = unsafe {
                    FlushInstructionCache(self.handle, address as *const c_void, data.len())
                };
                if flushed == 0 {
                    return Err(RuntimeError::InstructionCacheFlush {
                        address,
                        size: data.len(),
                        code: last_error(),
                    });
                }
                Ok(())
            });
            // Restore runs even when the write failed, so a failed mutation never
            // leaves the page writable.
            let mut restored: u32 = 0;
            unsafe {
                VirtualProtectEx(
                    self.handle,
                    address as *const c_void,
                    data.len(),
                    previous,
                    &mut restored,
                )
            };
            result
        }

        fn allocate_executable_near(
            &mut self,
            address: u64,
            size: usize,
        ) -> Result<u64, RuntimeError> {
            // Port of `_allocate_near`: walk free regions and never accept an
            // allocation outside the rel32 jump range of the hook.
            let lower = std::cmp::max(ALLOCATION_GRANULARITY, address.saturating_sub(REL32_REACH));
            let upper = std::cmp::min(0x7FFF_FFFF_FFFF, address.saturating_add(REL32_REACH));
            let mut cursor = lower;
            while cursor < upper {
                let mut information: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
                let queried = unsafe {
                    VirtualQueryEx(
                        self.handle,
                        cursor as *const c_void,
                        &mut information,
                        std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
                    )
                };
                if queried == 0 {
                    cursor = cursor.saturating_add(ALLOCATION_GRANULARITY);
                    continue;
                }
                let base = information.BaseAddress as u64;
                let region_end = base.saturating_add(information.RegionSize as u64);
                if information.State == MEM_FREE {
                    let candidate = align_up(std::cmp::max(cursor, base), ALLOCATION_GRANULARITY);
                    if candidate + size as u64 <= std::cmp::min(region_end, upper) {
                        let allocation = unsafe {
                            VirtualAllocEx(
                                self.handle,
                                candidate as *const c_void,
                                size,
                                MEM_COMMIT | MEM_RESERVE,
                                PAGE_EXECUTE_READWRITE,
                            )
                        };
                        if !allocation.is_null() {
                            let result = allocation as u64;
                            crate::mutation::trampoline::build_relative_jump(address, result)?;
                            return Ok(result);
                        }
                    }
                }
                cursor = std::cmp::max(cursor.saturating_add(ALLOCATION_GRANULARITY), region_end);
            }
            Err(RuntimeError::AllocationUnavailable {
                address,
                size: size as u64,
            })
        }

        fn free_allocation(&mut self, address: u64) -> Result<(), RuntimeError> {
            let freed =
                unsafe { VirtualFreeEx(self.handle, address as *mut c_void, 0, MEM_RELEASE) };
            if freed == 0 {
                return Err(RuntimeError::AllocationRelease {
                    address,
                    code: last_error(),
                });
            }
            Ok(())
        }

        fn exited(&mut self) -> Result<bool, RuntimeError> {
            let mut code: u32 = 0;
            if unsafe { GetExitCodeProcess(self.handle, &mut code) } == 0 {
                // Unknown, not exited: the caller must retain ownership.
                return Ok(false);
            }
            Ok(code != STILL_ACTIVE)
        }

        fn creation_filetime(&mut self) -> Result<Option<u64>, RuntimeError> {
            let mut created = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let mut exited = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let mut kernel = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let mut user = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let ok = unsafe {
                GetProcessTimes(
                    self.handle,
                    &mut created,
                    &mut exited,
                    &mut kernel,
                    &mut user,
                )
            };
            if ok == 0 {
                return Err(RuntimeError::ProcessQuery {
                    pid: self.pid,
                    code: last_error(),
                    detail: "GetProcessTimes",
                });
            }
            Ok(Some(
                ((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64,
            ))
        }

        fn close(&mut self) {
            if !self.handle.is_null() && self.handle != INVALID_HANDLE_VALUE {
                unsafe { CloseHandle(self.handle) };
                self.handle = std::ptr::null_mut::<c_void>();
            }
        }
    }
}
