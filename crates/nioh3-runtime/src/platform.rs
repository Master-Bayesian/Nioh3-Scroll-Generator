//! Windows read-only process, module and image-version adapter.
//!
//! Ports the read surface the shipped runtime host uses:
//!
//! | Shipped Python | This module |
//! | --- | --- |
//! | `native.find_nioh3_pids` / `find_nioh3_pid` | [`discover_process_ids`] / [`single_process_id`] |
//! | `game_compatibility._file_version` | [`file_version`] |
//! | `game_compatibility.verify_game_executable` | [`verify_game_executable`] |
//! | `native.find_module_base` | [`module_range`] (base and size) |
//! | `process_instance.creation_time_from_handle` | [`process_creation_filetime`] |
//! | `runtime_application.running_game_identity` | [`identify_running_game`] |
//! | `ProcessReader` plus `NativeBatchOracle.open` checks | [`ValidatedProcess`] |
//!
//! Access rights are minimal and read-only: `PROCESS_QUERY_LIMITED_INFORMATION`
//! (`0x1000`) for identity, and `PROCESS_QUERY_INFORMATION | PROCESS_VM_READ`
//! for the validated reader. No write right is ever requested.

use crate::profile::NativeRuntimeProfile;

/// Image name of the game process.
pub const GAME_IMAGE_NAME: &str = "Nioh3.exe";
/// Module name of the game image inside the process.
pub const GAME_MODULE_NAME: &str = "Nioh3.exe";

/// `PROCESS_QUERY_LIMITED_INFORMATION`. Enough for image path, exit code and
/// creation time; it is what `runtime_application.running_game_identity` uses.
pub const IDENTITY_ACCESS: u32 = 0x1000;
/// `PROCESS_QUERY_INFORMATION | PROCESS_VM_READ`. Read-only memory access.
pub const READER_ACCESS: u32 = 0x0400 | 0x0010;

const STILL_ACTIVE: u32 = 259;
const ERROR_INVALID_PARAMETER: u32 = 87;

/// Fixed file version, in `(major, minor, build, revision)` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileVersion {
    pub major: u16,
    pub minor: u16,
    pub build: u16,
    pub revision: u16,
}

impl FileVersion {
    pub const fn new(major: u16, minor: u16, build: u16, revision: u16) -> Self {
        Self {
            major,
            minor,
            build,
            revision,
        }
    }

    pub const fn tuple(self) -> (u16, u16, u16, u16) {
        (self.major, self.minor, self.build, self.revision)
    }

    /// `2.0.1.0` style display, matching `".".join(str(part) for part in version)`.
    pub fn display(self) -> String {
        format!(
            "{}.{}.{}.{}",
            self.major, self.minor, self.build, self.revision
        )
    }
}

/// `game_compatibility.GameCompatibilityStatus.state`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameCompatibility {
    Supported,
    Unsupported,
    Unreadable,
}

impl GameCompatibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
            Self::Unreadable => "unreadable",
        }
    }
}

/// Result of inspecting one executable's fixed version resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameExecutableStatus {
    pub state: GameCompatibility,
    pub file_version: Option<FileVersion>,
    pub executable: String,
}

impl GameExecutableStatus {
    pub fn supported(&self) -> bool {
        self.state == GameCompatibility::Supported && self.file_version.is_some()
    }
}

/// A process instance: the pid plus the creation FILETIME that disambiguates a
/// reused Windows pid. `ProcessReader.creation_time` is the shipped equivalent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub creation_filetime: u64,
}

/// Loaded module image extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleRange {
    pub base: u64,
    pub size: u64,
}

impl ModuleRange {
    /// True when `[offset, offset + length)` fits inside the image.
    pub fn contains_offset(&self, offset: u64, length: u64) -> bool {
        offset
            .checked_add(length)
            .is_some_and(|end| end <= self.size)
    }

    /// Absolute address of a module-relative offset.
    pub fn absolute(&self, offset: u64) -> Option<u64> {
        self.base.checked_add(offset)
    }
}

/// Everything `running_game_identity` returns, with the process instance added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameIdentity {
    pub identity: ProcessIdentity,
    pub executable: String,
    pub file_version: FileVersion,
    pub module: ModuleRange,
    pub profile: NativeRuntimeProfile,
}

#[cfg(windows)]
mod imp {
    use super::{
        FileVersion, GameCompatibility, GameExecutableStatus, GameIdentity, ModuleRange,
        ProcessIdentity, ERROR_INVALID_PARAMETER, IDENTITY_ACCESS, READER_ACCESS, STILL_ACTIVE,
    };
    use crate::error::RuntimeError;
    use crate::profile::{
        profile_for_game_version, supported_display_version, NativeRuntimeProfile,
    };
    use std::path::Path;
    use std::ptr;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, FILETIME, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW,
        MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, GetProcessTimes, OpenProcess, QueryFullProcessImageNameW,
    };

    /// RAII wrapper so no code path can leak a kernel handle.
    pub(super) struct OwnedHandle(HANDLE);

    impl OwnedHandle {
        fn from_raw(handle: HANDLE) -> Option<Self> {
            if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                None
            } else {
                Some(Self(handle))
            }
        }

        fn raw(&self) -> HANDLE {
            self.0
        }
    }

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // Closing a handle never touches the target process.
            unsafe { CloseHandle(self.0) };
        }
    }

    // A process handle is process-wide; sharing it across threads only lets a
    // worker read the same read-only view.
    unsafe impl Send for OwnedHandle {}

    fn last_error() -> u32 {
        unsafe { GetLastError() }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn decode_wide(units: &[u16]) -> String {
        let end = units
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(units.len());
        String::from_utf16_lossy(&units[..end])
    }

    fn open_process(pid: u32, access: u32) -> Result<OwnedHandle, RuntimeError> {
        let handle = unsafe { OpenProcess(access, 0, pid) };
        OwnedHandle::from_raw(handle).ok_or_else(|| RuntimeError::OpenProcess {
            pid,
            code: last_error(),
        })
    }

    fn creation_time_from_handle(pid: u32, handle: HANDLE) -> Result<u64, RuntimeError> {
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
        let ok =
            unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) };
        if ok == 0 {
            return Err(RuntimeError::ProcessQuery {
                pid,
                code: last_error(),
                detail: "GetProcessTimes",
            });
        }
        Ok(((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64)
    }

    /// `process_instance.process_creation_time`: the creation FILETIME of a live
    /// process, or `None` when the pid no longer names one.
    pub fn process_creation_filetime(pid: u32) -> Result<Option<u64>, RuntimeError> {
        let handle = match open_process(pid, IDENTITY_ACCESS) {
            Ok(handle) => handle,
            Err(RuntimeError::OpenProcess { code, .. }) if code == ERROR_INVALID_PARAMETER => {
                return Ok(None)
            }
            Err(error) => return Err(error),
        };
        let mut exit_code: u32 = 0;
        if unsafe { GetExitCodeProcess(handle.raw(), &mut exit_code) } == 0 {
            return Err(RuntimeError::ProcessQuery {
                pid,
                code: last_error(),
                detail: "GetExitCodeProcess",
            });
        }
        if exit_code != STILL_ACTIVE {
            return Ok(None);
        }
        creation_time_from_handle(pid, handle.raw()).map(Some)
    }

    /// `native.find_nioh3_pids`: every match, so a single-owner policy stays the
    /// caller's decision.
    pub fn discover_process_ids(image_name: &str) -> Result<Vec<u32>, RuntimeError> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        let snapshot =
            OwnedHandle::from_raw(snapshot).ok_or_else(|| RuntimeError::ModuleSnapshot {
                pid: 0,
                code: last_error(),
            })?;
        let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut matches = Vec::new();
        let mut ok = unsafe { Process32FirstW(snapshot.raw(), &mut entry) };
        while ok != 0 {
            if decode_wide(&entry.szExeFile).eq_ignore_ascii_case(image_name) {
                matches.push(entry.th32ProcessID);
            }
            ok = unsafe { Process32NextW(snapshot.raw(), &mut entry) };
        }
        Ok(matches)
    }

    /// `native.find_nioh3_pid`: exactly one owner, or a typed absence.
    pub fn single_process_id(image_name: &str) -> Result<u32, RuntimeError> {
        let matches = discover_process_ids(image_name)?;
        match matches.as_slice() {
            [] => Err(RuntimeError::ProcessAbsent {
                image: image_name.to_string(),
            }),
            [only] => Ok(*only),
            many => Err(RuntimeError::AmbiguousProcess {
                image: image_name.to_string(),
                count: many.len(),
            }),
        }
    }

    /// `running_game_identity`'s executable path lookup.
    pub fn query_image_path(pid: u32) -> Result<String, RuntimeError> {
        let handle = open_process(pid, IDENTITY_ACCESS)?;
        let mut buffer = vec![0u16; 32768];
        let mut size = buffer.len() as u32;
        let ok =
            unsafe { QueryFullProcessImageNameW(handle.raw(), 0, buffer.as_mut_ptr(), &mut size) };
        if ok == 0 {
            return Err(RuntimeError::QueryImageName {
                pid,
                code: last_error(),
            });
        }
        let length = (size as usize).min(buffer.len());
        Ok(String::from_utf16_lossy(&buffer[..length]))
    }

    /// The fixed version resource, read exactly as `_file_version` reads it.
    pub fn file_version(path: &str) -> Result<FileVersion, RuntimeError> {
        let path_wide = wide(path);
        let mut ignored: u32 = 0;
        let size = unsafe { GetFileVersionInfoSizeW(path_wide.as_ptr(), &mut ignored) };
        if size == 0 {
            return Err(RuntimeError::FileVersionUnreadable {
                path: path.to_string(),
                code: last_error(),
            });
        }
        let mut buffer = vec![0u8; size as usize];
        let ok = unsafe {
            GetFileVersionInfoW(
                path_wide.as_ptr(),
                0,
                size,
                buffer.as_mut_ptr() as *mut core::ffi::c_void,
            )
        };
        if ok == 0 {
            return Err(RuntimeError::FileVersionUnreadable {
                path: path.to_string(),
                code: last_error(),
            });
        }
        let root = wide("\\");
        let mut value: *mut core::ffi::c_void = ptr::null_mut();
        let mut value_size: u32 = 0;
        let ok = unsafe {
            VerQueryValueW(
                buffer.as_ptr() as *const core::ffi::c_void,
                root.as_ptr(),
                &mut value,
                &mut value_size,
            )
        };
        if ok == 0
            || value.is_null()
            || (value_size as usize) < std::mem::size_of::<FixedFileInfo>()
        {
            return Err(RuntimeError::FileVersionUnreadable {
                path: path.to_string(),
                code: last_error(),
            });
        }
        let fixed = unsafe { ptr::read_unaligned(value as *const FixedFileInfo) };
        Ok(FileVersion::new(
            (fixed.file_version_ms >> 16) as u16,
            (fixed.file_version_ms & 0xFFFF) as u16,
            (fixed.file_version_ls >> 16) as u16,
            (fixed.file_version_ls & 0xFFFF) as u16,
        ))
    }

    /// `game_compatibility.verify_game_executable`: never raises; an unreadable
    /// resource and an unsupported version are different states.
    pub fn verify_game_executable(path: &str) -> GameExecutableStatus {
        match file_version(path) {
            Ok(version) => GameExecutableStatus {
                state: if supported_display_version(version).is_some() {
                    GameCompatibility::Supported
                } else {
                    GameCompatibility::Unsupported
                },
                file_version: Some(version),
                executable: path.to_string(),
            },
            Err(_) => GameExecutableStatus {
                state: GameCompatibility::Unreadable,
                file_version: None,
                executable: path.to_string(),
            },
        }
    }

    /// `native.find_module_base`, extended with the image size so a later read
    /// can be bounded before it happens.
    pub fn module_range(pid: u32, module_name: &str) -> Result<ModuleRange, RuntimeError> {
        let snapshot =
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) };
        let snapshot =
            OwnedHandle::from_raw(snapshot).ok_or_else(|| RuntimeError::ModuleSnapshot {
                pid,
                code: last_error(),
            })?;
        let mut entry: MODULEENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;
        let mut ok = unsafe { Module32FirstW(snapshot.raw(), &mut entry) };
        while ok != 0 {
            if decode_wide(&entry.szModule).eq_ignore_ascii_case(module_name) {
                return Ok(ModuleRange {
                    base: entry.modBaseAddr as u64,
                    size: entry.modBaseSize as u64,
                });
            }
            ok = unsafe { Module32NextW(snapshot.raw(), &mut entry) };
        }
        Err(RuntimeError::ModuleNotFound {
            pid,
            module: module_name.to_string(),
        })
    }

    /// Port of `runtime_application.running_game_identity`.
    pub fn identify_running_game_named(
        image_name: &str,
        module_name: &str,
        profile_dir: &Path,
    ) -> Result<GameIdentity, RuntimeError> {
        let pid = single_process_id(image_name)?;
        let executable = query_image_path(pid)?;
        let status = verify_game_executable(&executable);
        let version = match (status.state, status.file_version) {
            (GameCompatibility::Supported, Some(version)) => version,
            (state, _) => {
                return Err(RuntimeError::GameExecutableUnsupported {
                    path: executable,
                    state: state.as_str(),
                })
            }
        };
        let profile = profile_for_game_version(version, profile_dir)?;
        let module = module_range(pid, module_name)?;
        let creation_filetime =
            process_creation_filetime(pid)?.ok_or(RuntimeError::ProcessGone { pid })?;
        Ok(GameIdentity {
            identity: ProcessIdentity {
                pid,
                creation_filetime,
            },
            executable,
            file_version: version,
            module,
            profile,
        })
    }

    /// A read-only process view whose extent, identity and profile signatures
    /// were validated before any address was dereferenced.
    pub struct ValidatedProcess {
        identity: ProcessIdentity,
        module: ModuleRange,
        profile: NativeRuntimeProfile,
        profile_digest: String,
        handle: OwnedHandle,
    }

    impl ValidatedProcess {
        /// Open a validated read view.
        ///
        /// Order is part of the contract:
        /// 1. resolve the live creation FILETIME (absence is not a failure);
        /// 2. refuse a different process instance than `expected_creation`;
        /// 3. resolve the module extent;
        /// 4. bounds-check every profile site against that extent;
        /// 5. only then request `PROCESS_QUERY_INFORMATION | PROCESS_VM_READ`;
        /// 6. re-check the creation FILETIME on the handle actually used, so a
        ///    pid recycled between step 1 and step 5 cannot be read.
        pub fn open(
            pid: u32,
            module_name: &str,
            profile: NativeRuntimeProfile,
            expected_creation: Option<u64>,
        ) -> Result<Self, RuntimeError> {
            let creation =
                process_creation_filetime(pid)?.ok_or(RuntimeError::ProcessGone { pid })?;
            if let Some(expected) = expected_creation {
                if expected != creation {
                    return Err(RuntimeError::ProcessInstanceChanged { pid });
                }
            }
            let module = module_range(pid, module_name)?;
            profile.validate_site_bounds(module.size)?;
            let handle = open_process(pid, READER_ACCESS)?;
            let recheck = creation_time_from_handle(pid, handle.raw())?;
            if recheck != creation {
                return Err(RuntimeError::ProcessInstanceChanged { pid });
            }
            let profile_digest = profile.identity_digest();
            Ok(Self {
                identity: ProcessIdentity {
                    pid,
                    creation_filetime: creation,
                },
                module,
                profile,
                profile_digest,
                handle,
            })
        }

        pub fn identity(&self) -> ProcessIdentity {
            self.identity
        }

        pub fn module_range(&self) -> ModuleRange {
            self.module
        }

        pub fn profile(&self) -> &NativeRuntimeProfile {
            &self.profile
        }

        pub fn profile_digest(&self) -> &str {
            &self.profile_digest
        }

        /// Read `size` bytes at an absolute address inside `within`.
        pub fn read_at(
            &self,
            address: u64,
            size: usize,
            within: ModuleRange,
        ) -> Result<Vec<u8>, RuntimeError> {
            let length = size as u64;
            let inside = address >= within.base
                && address
                    .checked_add(length)
                    .zip(within.base.checked_add(within.size))
                    .is_some_and(|(end, limit)| end <= limit);
            if !inside {
                return Err(RuntimeError::RangeOutOfBounds {
                    offset: address.saturating_sub(within.base),
                    size: length,
                    limit: within.size,
                });
            }
            self.read_checked(address, size)
        }

        /// Read `size` bytes at a module-relative offset.
        pub fn read_module(&self, offset: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
            let address = self
                .module
                .absolute(offset)
                .ok_or(RuntimeError::RangeOutOfBounds {
                    offset,
                    size: size as u64,
                    limit: self.module.size,
                })?;
            self.read_at(address, size, self.module)
        }

        fn read_checked(&self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
            if size == 0 {
                return Ok(Vec::new());
            }
            let mut buffer = vec![0u8; size];
            let mut read: usize = 0;
            let ok = unsafe {
                ReadProcessMemory(
                    self.handle.raw(),
                    address as *const core::ffi::c_void,
                    buffer.as_mut_ptr() as *mut core::ffi::c_void,
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

        /// Verify the captured signatures of the validated profile.
        ///
        /// Order matches `NativeBatchOracle.open`: canonicalize, finalizer, then
        /// the eight chain sites. Returns the number of verified sites.
        pub fn verify_profile_signatures(&self) -> Result<usize, RuntimeError> {
            let sites = self.profile.verification_sites();
            for site in &sites {
                let actual = self.read_module(site.rva, site.signature.len())?;
                if actual != site.signature {
                    return Err(RuntimeError::SignatureMismatch {
                        site: site.name.to_string(),
                        rva: site.rva,
                    });
                }
            }
            Ok(sites.len())
        }
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    struct FixedFileInfo {
        signature: u32,
        structure_version: u32,
        file_version_ms: u32,
        file_version_ls: u32,
        product_version_ms: u32,
        product_version_ls: u32,
        file_flags_mask: u32,
        file_flags: u32,
        file_os: u32,
        file_type: u32,
        file_subtype: u32,
        file_date_ms: u32,
        file_date_ls: u32,
    }

    /// Convenience wrapper using the shipped game image and module names.
    pub fn identify_running_game(profile_dir: &Path) -> Result<GameIdentity, RuntimeError> {
        identify_running_game_named(super::GAME_IMAGE_NAME, super::GAME_MODULE_NAME, profile_dir)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{GameExecutableStatus, GameIdentity, ModuleRange, ProcessIdentity};
    use crate::error::RuntimeError;
    use crate::profile::NativeRuntimeProfile;
    use std::path::Path;

    pub fn process_creation_filetime(_pid: u32) -> Result<Option<u64>, RuntimeError> {
        Err(RuntimeError::UnsupportedPlatform)
    }

    pub fn discover_process_ids(_image_name: &str) -> Result<Vec<u32>, RuntimeError> {
        Err(RuntimeError::UnsupportedPlatform)
    }

    pub fn single_process_id(_image_name: &str) -> Result<u32, RuntimeError> {
        Err(RuntimeError::UnsupportedPlatform)
    }

    pub fn query_image_path(_pid: u32) -> Result<String, RuntimeError> {
        Err(RuntimeError::UnsupportedPlatform)
    }

    pub fn file_version(_path: &str) -> Result<super::FileVersion, RuntimeError> {
        Err(RuntimeError::UnsupportedPlatform)
    }

    pub fn verify_game_executable(path: &str) -> GameExecutableStatus {
        GameExecutableStatus {
            state: super::GameCompatibility::Unreadable,
            file_version: None,
            executable: path.to_string(),
        }
    }

    pub fn module_range(_pid: u32, _module_name: &str) -> Result<ModuleRange, RuntimeError> {
        Err(RuntimeError::UnsupportedPlatform)
    }

    pub fn identify_running_game_named(
        _image_name: &str,
        _module_name: &str,
        _profile_dir: &Path,
    ) -> Result<GameIdentity, RuntimeError> {
        Err(RuntimeError::UnsupportedPlatform)
    }

    pub fn identify_running_game(_profile_dir: &Path) -> Result<GameIdentity, RuntimeError> {
        Err(RuntimeError::UnsupportedPlatform)
    }

    /// Placeholder so the crate compiles off Windows; every constructor fails.
    pub struct ValidatedProcess {
        _process_identity: ProcessIdentity,
    }

    impl ValidatedProcess {
        pub fn open(
            _pid: u32,
            _module_name: &str,
            _profile: NativeRuntimeProfile,
            _expected_creation: Option<u64>,
        ) -> Result<Self, RuntimeError> {
            Err(RuntimeError::UnsupportedPlatform)
        }

        pub fn identity(&self) -> ProcessIdentity {
            self._process_identity
        }

        pub fn verify_profile_signatures(&self) -> Result<usize, RuntimeError> {
            Err(RuntimeError::UnsupportedPlatform)
        }
    }
}

pub use imp::*;

// Real Windows read-only API probes against this test process. They need no
// game, no Cheat Engine and no elevation, and they never write to any process.
#[cfg(all(test, windows))]
mod windows_tests {
    use super::{
        discover_process_ids, module_range, process_creation_filetime, single_process_id,
        ValidatedProcess,
    };
    use crate::error::RuntimeError;
    use crate::profile::{default_pc_v2_00_02, NativeRuntimeProfile, ProfileSite};

    fn current_image_name() -> String {
        std::env::current_exe()
            .ok()
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_default()
    }

    /// A profile whose three verified sites all point at the image header, so a
    /// small test binary can satisfy the bounds check.
    fn header_profile(signature: Vec<u8>) -> NativeRuntimeProfile {
        let mut profile = default_pc_v2_00_02();
        profile.native_signatures.clear();
        for site in [
            &mut profile.canonicalize,
            &mut profile.finalize_effect,
            &mut profile.descriptor_complete,
        ] {
            *site = ProfileSite {
                name: site.name,
                rva: 0,
                signature: signature.clone(),
            };
        }
        profile.playthrough_selector_pointer_rva = 0;
        profile
    }

    #[test]
    fn discovery_and_single_owner_resolution_work_on_a_live_process() -> Result<(), RuntimeError> {
        let image = current_image_name();
        assert!(!image.is_empty(), "the test binary has a name");
        let found = discover_process_ids(&image)?;
        assert!(found.contains(&std::process::id()), "{found:?}");
        assert_eq!(single_process_id(&image)?, std::process::id());
        assert_eq!(
            single_process_id("no-such-image-8f2c.exe").err(),
            Some(RuntimeError::ProcessAbsent {
                image: "no-such-image-8f2c.exe".to_string(),
            })
        );
        Ok(())
    }

    #[test]
    fn a_dead_pid_is_absence_while_a_live_pid_has_a_stable_identity() -> Result<(), RuntimeError> {
        let pid = std::process::id();
        let first = process_creation_filetime(pid)?;
        assert!(first.is_some(), "the test process is alive");
        assert_eq!(first, process_creation_filetime(pid)?);
        assert_eq!(process_creation_filetime(u32::MAX - 1)?, None);
        Ok(())
    }

    #[test]
    fn module_ranges_and_site_bounds_precede_any_read() -> Result<(), RuntimeError> {
        let pid = std::process::id();
        let image = current_image_name();
        let range = module_range(pid, &image)?;
        assert!(range.base > 0 && range.size > 0);
        assert_eq!(
            module_range(pid, "not-a-loaded-module-8f2c.dll").err(),
            Some(RuntimeError::ModuleNotFound {
                pid,
                module: "not-a-loaded-module-8f2c.dll".to_string(),
            })
        );

        // The shipped game profile reaches past a test binary's image, so the
        // range check must refuse the read view before it opens a read handle.
        let creation = process_creation_filetime(pid)?;
        assert_eq!(
            ValidatedProcess::open(pid, &image, default_pc_v2_00_02(), creation).err(),
            Some(RuntimeError::RangeOutOfBounds {
                offset: default_pc_v2_00_02().canonicalize.rva,
                size: default_pc_v2_00_02().canonicalize.signature.len() as u64,
                limit: range.size,
            })
        );
        Ok(())
    }

    #[test]
    fn a_replaced_process_instance_is_refused_before_the_image_is_touched(
    ) -> Result<(), RuntimeError> {
        let pid = std::process::id();
        let image = current_image_name();
        assert_eq!(
            ValidatedProcess::open(pid, &image, default_pc_v2_00_02(), Some(0)).err(),
            Some(RuntimeError::ProcessInstanceChanged { pid })
        );
        Ok(())
    }

    #[test]
    fn the_validated_read_view_reads_and_verifies_this_image() -> Result<(), RuntimeError> {
        let pid = std::process::id();
        let image = current_image_name();
        let creation = process_creation_filetime(pid)?;

        let probe = ValidatedProcess::open(pid, &image, header_profile(vec![0]), creation)?;
        let header = probe.read_module(0, 8)?;
        assert_eq!(header.len(), 8);
        assert_eq!(
            probe.read_module(probe.module_range().size - 1, 2).err(),
            Some(RuntimeError::RangeOutOfBounds {
                offset: probe.module_range().size - 1,
                size: 2,
                limit: probe.module_range().size,
            }),
            "a read past the image is refused before it happens"
        );
        assert_eq!(probe.identity().pid, pid);
        assert_eq!(probe.profile_digest().len(), 64);

        let verified = ValidatedProcess::open(pid, &image, header_profile(header), creation)?;
        // The eight chain sites were cleared, so canonicalize and the finalizer
        // are the two verified sites.
        assert_eq!(verified.verify_profile_signatures()?, 2);
        assert_eq!(
            ValidatedProcess::open(pid, &image, header_profile(vec![0xAA]), creation)?
                .verify_profile_signatures()
                .err(),
            Some(RuntimeError::SignatureMismatch {
                site: "canonicalize".to_string(),
                rva: 0,
            })
        );
        Ok(())
    }
}
