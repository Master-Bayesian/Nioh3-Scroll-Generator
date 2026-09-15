//! Real accelerator capability probes for the handshake.
//!
//! The shipped Python worker answers `cuda_seed_acceleration_available()` and
//! `d3d11_effect_acceleration_available()` by loading the shipped helper DLLs
//! and calling their exported probes. This module does the same rather than
//! reporting a constant, so the advertised capabilities change with what is
//! actually loadable on this machine. Only read-only probe exports are called;
//! no execution policy is installed here.

use std::path::{Path, PathBuf};

/// Whether the DirectCompute effect-filter route itself is ported.
///
/// The helper DLL probe is real, but the wire capability reports availability
/// intersected with what this worker can actually do: advertising `true` while
/// the route is unported would promise a usable filter to the client. The
/// partial-effect forward filter now serves the shipped surface
/// (`Route::PartialEffectFilter` plus the certified recomposition), and its gate
/// compares candidate identity, per-candidate cursors and the page cursor
/// against the shipped worker, so the intersection is the probe again.
pub const EFFECT_FILTER_ROUTE_PORTED: bool = true;

/// The published effect-filter capability: the helper probe intersected with
/// what this worker can actually run.
pub fn wire_effect_filter(probe: bool) -> bool {
    probe && EFFECT_FILTER_ROUTE_PORTED
}

/// Capability flags the handshake publishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// `seed_accelerator.cuda_seed_acceleration_available()`.
    pub cuda_pivot_and_auxiliary: bool,
    /// Wire value: the effect-filter probe intersected with
    /// [`EFFECT_FILTER_ROUTE_PORTED`].
    pub directcompute_effect_filter: bool,
    /// The raw `d3d11_effect_acceleration_available(0)` probe, kept for internal
    /// diagnostics and never published as an implemented capability.
    pub directcompute_probe: bool,
    /// Bulk CPU work is never implicit; it needs the explicit job opt-in.
    pub bulk_cpu_requires_opt_in: bool,
}

/// `bin/nioh3_seed_accelerator.dll` next to the application root, or the
/// caller-supplied override.
pub fn seed_accelerator_path(application_root: &Path, override_path: Option<&Path>) -> PathBuf {
    match override_path {
        Some(path) => path.to_path_buf(),
        None => application_root
            .join("bin")
            .join("nioh3_seed_accelerator.dll"),
    }
}

/// `bin/nioh3_effect_preimage_accelerator.dll`, or `NIOH3_EFFECT_PREIMAGE_ACCELERATOR`.
pub fn effect_preimage_path(application_root: &Path, override_path: Option<&Path>) -> PathBuf {
    if let Some(path) = override_path {
        return path.to_path_buf();
    }
    match std::env::var_os("NIOH3_EFFECT_PREIMAGE_ACCELERATOR") {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => application_root
            .join("bin")
            .join("nioh3_effect_preimage_accelerator.dll"),
    }
}

/// Probe both helpers. A missing or unloadable DLL reports `false`, exactly like
/// the Python probes, so a caller can never be promised acceleration the worker
/// cannot load.
pub fn probe(
    application_root: &Path,
    seed_override: Option<&Path>,
    preimage_override: Option<&Path>,
) -> Capabilities {
    platform::probe(
        &seed_accelerator_path(application_root, seed_override),
        &effect_preimage_path(application_root, preimage_override),
    )
}

#[cfg(not(windows))]
mod platform {
    use std::path::Path;

    use super::Capabilities;

    pub fn probe(_seed: &Path, _preimage: &Path) -> Capabilities {
        Capabilities {
            cuda_pivot_and_auxiliary: false,
            directcompute_effect_filter: false,
            directcompute_probe: false,
            bulk_cpu_requires_opt_in: true,
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::ffi::{c_void, OsStr};
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use super::Capabilities;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
        fn FreeLibrary(module: *mut c_void) -> i32;
    }

    type SeedProbe = unsafe extern "C" fn() -> i32;
    type PreimageProbe = unsafe extern "C" fn(u32) -> i32;

    /// Load one helper and call the named probe export.
    ///
    /// # Safety
    ///
    /// The loaded library is our own shipped accelerator helper; every symbol
    /// is looked up by name and only called through its documented signature.
    unsafe fn probe_export<T: Copy>(path: &Path, symbol: &[u8]) -> Option<T> {
        if !path.is_file() {
            return None;
        }
        let wide: Vec<u16> = OsStr::new(path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let module = LoadLibraryW(wide.as_ptr());
        if module.is_null() {
            return None;
        }
        let address = GetProcAddress(module, symbol.as_ptr());
        if address.is_null() {
            FreeLibrary(module);
            return None;
        }
        Some(std::mem::transmute_copy::<*mut c_void, T>(&address))
    }

    pub fn probe(seed: &Path, preimage: &Path) -> Capabilities {
        let cuda = unsafe {
            // SAFETY: `cuda_seed_acceleration_available` takes no arguments and
            // returns `int`, matching `seed_accelerator.py`.
            probe_export::<SeedProbe>(seed, b"cuda_seed_acceleration_available\0")
                .map(|probe| probe() != 0)
                .unwrap_or(false)
        };
        let directcompute = unsafe {
            // SAFETY: `d3d11_effect_acceleration_available` takes the preferred
            // vendor id as `uint32` and returns `int`, matching
            // `effect_preimage_accelerator.py`'s default of `0`.
            probe_export::<PreimageProbe>(preimage, b"d3d11_effect_acceleration_available\0")
                .map(|probe| probe(0) != 0)
                .unwrap_or(false)
        };
        Capabilities {
            cuda_pivot_and_auxiliary: cuda,
            directcompute_effect_filter: super::wire_effect_filter(directcompute),
            directcompute_probe: directcompute,
            bulk_cpu_requires_opt_in: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_helper_path_reports_unavailable() {
        let missing = Path::new("D:/definitely-missing/none.dll");
        let probe = probe(
            Path::new("D:/definitely-missing"),
            Some(missing),
            Some(missing),
        );
        assert!(!probe.cuda_pivot_and_auxiliary);
        assert!(!probe.directcompute_effect_filter);
        assert!(!probe.directcompute_probe);
        assert!(probe.bulk_cpu_requires_opt_in);
    }

    #[test]
    fn the_effect_filter_capability_follows_the_ported_route() {
        // The wire value is the live probe intersected with the ported route.
        // With the route ported and the helper loadable the client is promised
        // the filter; with the helper absent it must not be advertised.
        assert_eq!(wire_effect_filter(true), EFFECT_FILTER_ROUTE_PORTED);
        assert!(!wire_effect_filter(false));
        let probed = Capabilities {
            cuda_pivot_and_auxiliary: true,
            directcompute_effect_filter: wire_effect_filter(true),
            directcompute_probe: true,
            bulk_cpu_requires_opt_in: true,
        };
        assert!(
            probed.directcompute_effect_filter,
            "a loadable helper with a ported route is the advertised filter"
        );
        assert!(
            probed.directcompute_probe,
            "the raw probe is still recorded"
        );
    }

    #[test]
    fn helper_paths_prefer_an_explicit_override() {
        let root = Path::new("C:/app");
        assert_eq!(
            seed_accelerator_path(root, Some(Path::new("D:/x.dll"))),
            PathBuf::from("D:/x.dll")
        );
        assert_eq!(
            seed_accelerator_path(root, None),
            root.join("bin").join("nioh3_seed_accelerator.dll")
        );
    }
}
