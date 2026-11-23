//! One Engine owns the exact named data buffers used by its CPU and card.
//! Lazy card initialization must not reopen newer bundle files.
use crate::engine::LoadError;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct BundleInputs(BTreeMap<&'static str, Arc<[u8]>>);

impl BundleInputs {
    pub(crate) fn load(dir: &Path, legacy_image: bool) -> Result<Self, LoadError> {
        Self::load_named(dir, legacy_image, true)
    }

    pub(crate) fn load_for_context(dir: &Path, legacy_image: bool) -> Result<Self, LoadError> {
        // The encoded context owns characters.bin; an unused sample file is
        // neither required nor part of this evaluation session's identity.
        Self::load_named(dir, legacy_image, false)
    }

    fn load_named(
        dir: &Path,
        legacy_image: bool,
        sample_character: bool,
    ) -> Result<Self, LoadError> {
        let mut files = BTreeMap::new();
        for name in [
            "programs.txt",
            "arena.bin",
            "tables.bin",
            "frame.bin",
            "characters.bin",
            "gpu_setup.txt",
            "hits.txt",
            "live_groups.txt",
            "survival_groups.txt",
        ] {
            if name == "characters.bin" && !sample_character {
                continue;
            }
            let bytes = crate::kernel_asset::read(dir, name).map_err(|e| LoadError {
                file: dir.join(name),
                reason: e.to_string(),
            })?;
            files.insert(name, Arc::from(bytes));
        }
        for name in [
            "mapped.bin",
            "image.bin",
            "event_plan.json",
            "source_numeric.bin",
        ] {
            if name == "image.bin" && !legacy_image {
                continue;
            }
            match crate::kernel_asset::read(dir, name) {
                Ok(bytes) => {
                    files.insert(name, Arc::from(bytes));
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(LoadError {
                        file: dir.join(name),
                        reason: e.to_string(),
                    })
                }
            }
        }
        Ok(Self(files))
    }

    pub(crate) fn file(&self, name: &str) -> Option<&[u8]> {
        self.0.get(name).map(AsRef::as_ref)
    }

    pub(crate) fn required(&self, name: &str) -> &[u8] {
        self.file(name)
            .expect("BundleInputs::load checked required file")
    }

    pub(crate) fn digest(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        for (name, bytes) in &self.0 {
            hash.update((name.len() as u64).to_le_bytes());
            hash.update(name.as_bytes());
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        format!("{:x}", hash.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lazy_consumers_keep_owned_bytes_after_the_file_changes() {
        let root = std::env::var_os("NIOH3_BUILD_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let dir = root.join(format!(
            "bundle-input-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for name in [
            "programs.txt",
            "arena.bin",
            "tables.bin",
            "frame.bin",
            "characters.bin",
            "gpu_setup.txt",
            "hits.txt",
            "live_groups.txt",
            "survival_groups.txt",
        ] {
            std::fs::write(dir.join(name), b"old").unwrap();
        }
        let owner = BundleInputs::load(&dir, true).unwrap();
        let lazy = owner.clone();
        std::fs::write(dir.join("hits.txt"), b"new").unwrap();
        std::fs::write(dir.join("image.bin"), b"later image").unwrap();
        assert_eq!(lazy.required("hits.txt"), b"old");
        assert_eq!(lazy.file("image.bin"), None);
        let new = BundleInputs::load(&dir, true).unwrap();
        assert_eq!(new.required("hits.txt"), b"new");
        assert_ne!(owner.digest(), new.digest());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn pooled_asset_bytes_are_owned_and_missing_sealed_inputs_fail() {
        let root = std::env::var_os("NIOH3_BUILD_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!("pooled-input-test-{}", std::process::id()));
        let scene = root.join("loadout/attached");
        std::fs::create_dir_all(&scene).unwrap();
        for name in [
            "programs.txt",
            "arena.bin",
            "frame.bin",
            "gpu_setup.txt",
            "hits.txt",
            "live_groups.txt",
            "survival_groups.txt",
        ] {
            std::fs::write(scene.join(name), b"local").unwrap();
        }
        let hash = crate::context::sha256(b"good");
        let pool = root.join("loadout-kernels").join(&hash);
        std::fs::create_dir_all(&pool).unwrap();
        let descriptor = serde_json::json!({"files": {
            "tables.bin": {"sha256": hash, "bytes": 4},
            "source_numeric.bin": {"sha256": hash, "bytes": 4}
        }});
        std::fs::write(scene.join("runtime-contract.json"), descriptor.to_string()).unwrap();
        for name in ["tables.bin", "source_numeric.bin"] {
            std::fs::write(pool.join(name), b"good").unwrap();
        }
        let inputs = BundleInputs::load_for_context(&scene, false).unwrap();
        std::fs::write(pool.join("source_numeric.bin"), b"bad!").unwrap();
        assert!(BundleInputs::load_for_context(&scene, false).is_err());
        assert_eq!(inputs.required("source_numeric.bin"), b"good");
        std::fs::remove_file(pool.join("source_numeric.bin")).unwrap();
        assert!(BundleInputs::load_for_context(&scene, false).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
