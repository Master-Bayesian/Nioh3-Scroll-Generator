//! Installed scenes can share identical kernel assets by their sealed digest.
use std::io;
use std::path::{Path, PathBuf};

pub const POOLED_FILES: [&str; 6] = [
    "main_cpu.dll",
    "main_device.dll",
    "tables.bin",
    "source_numeric.bin",
    "live_groups.txt",
    "survival_groups.txt",
];

struct SharedAsset {
    path: PathBuf,
    digest: String,
    length: u64,
}

impl SharedAsset {
    fn read(&self) -> io::Result<Vec<u8>> {
        let bytes = std::fs::read(&self.path)
            .map_err(|error| invalid(format!("{}: {error}", self.path.display())))?;
        if self.length != bytes.len() as u64 || crate::context::sha256(&bytes) != self.digest {
            return Err(invalid(format!(
                "{} differs from the sealed scene",
                self.path.display()
            )));
        }
        Ok(bytes)
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn shared(scene: &Path, name: &str, local: &Path) -> io::Result<Option<SharedAsset>> {
    if local.is_file() || !POOLED_FILES.contains(&name) {
        return Ok(None);
    }
    let Some(parent) = scene
        .parent()
        .filter(|p| p.ends_with("loadout"))
        .and_then(Path::parent)
    else {
        return Ok(None);
    };
    let pool = parent.join("loadout-kernels");
    if !pool.is_dir() {
        return Ok(None);
    }
    let contract: serde_json::Value = serde_json::from_slice(
        &std::fs::read(scene.join("runtime-contract.json")).map_err(|e| invalid(e.to_string()))?,
    )
    .map_err(|e| invalid(e.to_string()))?;
    let entry = &contract["files"][name];
    let hash = entry["sha256"]
        .as_str()
        .filter(|h| {
            h.len() == 64
                && h.bytes()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        })
        .ok_or_else(|| invalid(format!("invalid sealed {name} digest")))?;
    let length = entry["bytes"]
        .as_u64()
        .ok_or_else(|| invalid(format!("invalid sealed {name} length")))?;
    Ok(Some(SharedAsset {
        path: pool.join(hash).join(name),
        digest: hash.into(),
        length,
    }))
}

pub fn resolve(scene: &Path, name: &str) -> Result<PathBuf, String> {
    let local = scene.join(name);
    if let Some(asset) = shared(scene, name, &local).map_err(|error| error.to_string())? {
        asset.read().map_err(|error| error.to_string())?;
        return Ok(asset.path);
    }
    Ok(local)
}

pub fn read(scene: &Path, name: &str) -> io::Result<Vec<u8>> {
    let local = scene.join(name);
    match shared(scene, name, &local)? {
        Some(asset) => asset.read(),
        None => std::fs::read(local),
    }
}
