//! Native accelerator identity probe.
//!
//! Mirrors `nioh3_scroll_editor/seed_accelerator.py`: the optional
//! `nioh3_seed_accelerator.dll` is loaded, its exported ABI must be 2, it must
//! accept the authoritative execution policy (strict GPU unless an
//! operation-scoped guard holds an opt-in), and only then do its ABI number and
//! build identity join the generation context. A missing, ABI-mismatched or
//! policy-rejecting library contributes `None` instead of a fabricated value.

use std::path::Path;

/// Loaded accelerator identity, mirroring `seed_accelerator_identity()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceleratorIdentity {
    pub abi: i32,
    pub build_id: String,
}

/// ABI version the product accepts (`SEED_ACCELERATOR_ABI_VERSION`).
pub const SEED_ACCELERATOR_ABI_VERSION: i32 = 2;
/// `EXECUTION_POLICY_STRICT_GPU`, the policy in force while no guard is active.
pub const EXECUTION_POLICY_STRICT_GPU: i32 = 0;

#[cfg(windows)]
mod platform {
    use std::ffi::{c_void, CStr};
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use super::{AcceleratorIdentity, SEED_ACCELERATOR_ABI_VERSION};

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }

    /// Exported symbols the identity probe needs, in `ctypes` lookup order.
    type AbiVersion = unsafe extern "C" fn() -> i32;
    type BuildId = unsafe extern "C" fn() -> *const i8;
    type SetExecutionPolicy = unsafe extern "C" fn(i32) -> i32;

    pub(super) fn probe(module_path: &Path) -> Option<AcceleratorIdentity> {
        if !module_path.is_file() {
            return None;
        }
        let mut wide: Vec<u16> = module_path.as_os_str().encode_wide().collect();
        wide.push(0);
        // SAFETY: `wide` is a NUL-terminated UTF-16 path that stays alive for
        // the call. The returned handle is deliberately never freed, matching
        // the Python module's cached `ctypes.WinDLL`.
        let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
        if handle.is_null() {
            return None;
        }
        let abi_version: AbiVersion = unsafe { symbol(handle, b"seed_accelerator_abi_version\0")? };
        let build_id: BuildId = unsafe { symbol(handle, b"seed_accelerator_build_id\0")? };
        let set_policy: SetExecutionPolicy =
            unsafe { symbol(handle, b"seed_accelerator_set_execution_policy\0")? };

        if unsafe { abi_version() } != SEED_ACCELERATOR_ABI_VERSION {
            return None;
        }
        // The product starts from strict GPU, but the process-global policy may
        // already belong to an operation-scoped guard. Re-installing the
        // authoritative policy - read and written under one lock acquisition -
        // keeps the probe from cancelling that opt-in.
        let accepted =
            crate::native_search::reinstate_active_policy(|policy| unsafe { set_policy(policy) });
        if accepted != 0 {
            return None;
        }
        let raw = unsafe { build_id() };
        let build_id = if raw.is_null() {
            String::new()
        } else {
            // SAFETY: the library contract returns a NUL-terminated ASCII
            // string that outlives this call; undecodable bytes are replaced.
            unsafe { CStr::from_ptr(raw) }
                .to_string_lossy()
                .into_owned()
        };
        Some(AcceleratorIdentity {
            abi: SEED_ACCELERATOR_ABI_VERSION,
            build_id,
        })
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

    use super::AcceleratorIdentity;

    pub(super) fn probe(_module_path: &Path) -> Option<AcceleratorIdentity> {
        None
    }
}

/// Probe the accelerator library path the product would load.
///
/// `override_path` is the `NIOH3_SEED_ACCELERATOR` value; when it is empty the
/// default `<application root>/bin/nioh3_seed_accelerator.dll` is probed.
pub fn probe_seed_accelerator(
    application_root: &Path,
    override_path: Option<&Path>,
) -> Option<AcceleratorIdentity> {
    let module_path = match override_path {
        Some(path) => path.to_path_buf(),
        None => application_root
            .join("bin")
            .join("nioh3_seed_accelerator.dll"),
    };
    platform::probe(&module_path)
}
