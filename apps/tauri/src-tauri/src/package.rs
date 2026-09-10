use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    io::Read,
    path::{Component, Path},
};

pub const ENTRY: &str = "Nioh3Studio.exe";
#[derive(Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}
#[derive(Deserialize)]
pub struct Manifest {
    pub schema: String,
    pub version: String,
    pub files: Vec<FileEntry>,
}
pub fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut bytes = [0; 65536];
    loop {
        let n = file.read(&mut bytes).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        digest.update(&bytes[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
pub fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':', '<', '>', '"', '|', '?', '*'])
        && path.split('/').all(|p| {
            let device = p
                .split('.')
                .next()
                .unwrap_or("")
                .trim_end()
                .to_ascii_uppercase();
            let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&device.as_str())
                || ["COM", "LPT"].iter().any(|prefix| {
                    device.strip_prefix(prefix).is_some_and(|n| {
                        ["1", "2", "3", "4", "5", "6", "7", "8", "9", "¹", "²", "³"].contains(&n)
                    })
                });
            !reserved
                && !p.is_empty()
                && p != "."
                && p != ".."
                && !p.ends_with([' ', '.'])
                && !p.chars().any(char::is_control)
        })
        && Path::new(path)
            .components()
            .all(|p| matches!(p, Component::Normal(_)))
}
pub fn verify(root: &Path) -> Result<Manifest, String> {
    let manifest_path = root.join("build-manifest.json");
    if std::fs::metadata(&manifest_path)
        .map_err(|e| e.to_string())?
        .len()
        > 4_194_304
    {
        return Err("PACKAGE_MANIFEST_TOO_LARGE".into());
    }
    let value: Manifest =
        serde_json::from_slice(&std::fs::read(manifest_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if value.schema != "nioh3-tauri-manifest/v1"
        || value.files.is_empty()
        || value.files.len() > 10000
    {
        return Err("PACKAGE_MANIFEST_INVALID".into());
    }
    let mut names = HashSet::new();
    let canonical = root.canonicalize().map_err(|e| e.to_string())?;
    for entry in &value.files {
        if !safe_relative(&entry.path) || !names.insert(entry.path.to_lowercase()) {
            return Err("PACKAGE_PATH_INVALID".into());
        }
        let file = root.join(&entry.path);
        let metadata = std::fs::symlink_metadata(&file).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || !file
                .canonicalize()
                .map_err(|e| e.to_string())?
                .starts_with(&canonical)
            || metadata.len() != entry.size
            || hash_file(&file)? != entry.sha256
        {
            return Err(format!("PACKAGE_FILE_MISMATCH: {}", entry.path));
        }
    }
    for name in [
        ENTRY,
        "worker/nioh3-search-worker.exe",
        "worker/nioh3-protected-worker.exe",
        "packages/contracts/request.schema.json",
        "packages/contracts/response.schema.json",
        "packages/contracts/protected-request.schema.json",
        "packages/contracts/protected-response.schema.json",
    ] {
        if !names.contains(&name.to_lowercase()) {
            return Err(format!("PACKAGE_FILE_UNLISTED: {name}"));
        }
    }
    Ok(value)
}
fn no_links(root: &Path) -> Result<Vec<String>, String> {
    fn linked(path: &Path) -> Result<bool, String> {
        let m = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            return Ok(m.file_attributes() & 0x400 != 0);
        }
        #[cfg(not(windows))]
        {
            Ok(m.file_type().is_symlink())
        }
    }
    fn walk(root: &Path, path: &Path, files: &mut Vec<String>) -> Result<(), String> {
        if linked(path)? {
            return Err("CLEANUP_LINK_REFUSED".into());
        }
        for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if linked(&entry.path())? {
                return Err("CLEANUP_LINK_REFUSED".into());
            }
            if kind.is_dir() {
                walk(root, &entry.path(), files)?;
            } else if kind.is_file() {
                files.push(
                    entry
                        .path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            } else {
                return Err("CLEANUP_FILE_REFUSED".into());
            }
        }
        Ok(())
    }
    let mut files = vec![];
    walk(root, root, &mut files)?;
    Ok(files)
}
pub fn remove_child(parent: &Path, child: &Path, product_only: bool) -> Result<(), String> {
    if !child.exists() {
        return Ok(());
    }
    let canonical_parent = parent.canonicalize().map_err(|e| e.to_string())?;
    let canonical_child = child.canonicalize().map_err(|e| e.to_string())?;
    if canonical_child.parent() != Some(canonical_parent.as_path()) {
        return Err("CLEANUP_PATH_REFUSED".into());
    }
    let files = no_links(child)?;
    if product_only {
        let manifest = verify(child)?;
        let allowed: HashSet<_> = manifest
            .files
            .iter()
            .map(|f| f.path.as_str())
            .chain(["build-manifest.json"])
            .collect();
        if files.iter().any(|f| !allowed.contains(f.as_str())) {
            return Err("UPDATE_PREVIOUS_HAS_USER_FILES".into());
        }
    }
    std::fs::remove_dir_all(child).map_err(|e| e.to_string())
}
pub fn extract(archive: &Path, target: &Path) -> Result<(), String> {
    if target.exists() {
        return Err("UPDATE_STAGE_EXISTS".into());
    }
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    if zip.len() > 10000 {
        return Err("UPDATE_TOO_MANY_FILES".into());
    }
    let mut names = HashSet::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().trim_end_matches('/');
        total = total.checked_add(entry.size()).ok_or("UPDATE_TOO_LARGE")?;
        if !safe_relative(name)
            || !names.insert(name.to_lowercase())
            || entry.unix_mode().is_some_and(|m| m & 0xf000 == 0xa000)
            || total > 1_073_741_824
        {
            return Err("UPDATE_ARCHIVE_INVALID".into());
        }
    }
    std::fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let path = target.join(entry.name());
        if entry.is_dir() {
            std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
            continue;
        }
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
    }
    Ok(())
}
