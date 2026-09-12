//! Install-free, hash-verified execution of the portable product carried in the EXE.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs::{self, File, OpenOptions, TryLockError},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const MAGIC: &[u8; 16] = b"NIOH3_ONEFILE_V1";
pub const FOOTER_SIZE: u64 = 56;
pub const ENTRY: &str = "Nioh3Studio.exe";
pub const STUB: &str = "launcher/Nioh3Launcher.exe";
const MAX_ZIP_BYTES: u64 = 512 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_FILES: usize = 10_000;
const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const MAX_CACHES: usize = 2;
const MAX_CACHE_BYTES: u64 = 2 * MAX_UNPACKED_BYTES;
const MARKER: &str = ".onefile-cache.json";
const STAGING_MARKER: &str = ".onefile-stage.json";
const LEASE: &str = ".lease";
type Result<T> = std::result::Result<T, String>;

fn io_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn sha256(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

/// The source file remains pinned against Windows replacement until extraction ends.
pub struct Payload {
    file: File,
    offset: u64,
    length: u64,
    pub digest: String,
}
impl Payload {
    pub fn open(path: &Path) -> Result<Self> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1); // FILE_SHARE_READ, no rewrite during hash/extraction.
        }
        let mut file = options.open(path).map_err(io_error)?;
        let size = file.metadata().map_err(io_error)?.len();
        if size <= FOOTER_SIZE {
            return Err("ONEFILE_FOOTER_MISSING".into());
        }
        file.seek(SeekFrom::End(-(FOOTER_SIZE as i64)))
            .map_err(io_error)?;
        let mut footer = [0; FOOTER_SIZE as usize];
        file.read_exact(&mut footer).map_err(io_error)?;
        if &footer[..16] != MAGIC {
            return Err("ONEFILE_FOOTER_INVALID".into());
        }
        let length = u64::from_le_bytes(footer[16..24].try_into().unwrap());
        if length == 0 || length > MAX_ZIP_BYTES || length >= size - FOOTER_SIZE {
            return Err("ONEFILE_PAYLOAD_LENGTH_INVALID".into());
        }
        let offset = size - FOOTER_SIZE - length;
        file.seek(SeekFrom::Start(offset)).map_err(io_error)?;
        let mut digest = Sha256::new();
        let mut remaining = length;
        let mut bytes = [0; 65536];
        while remaining != 0 {
            let size = remaining.min(bytes.len() as u64) as usize;
            file.read_exact(&mut bytes[..size]).map_err(io_error)?;
            digest.update(&bytes[..size]);
            remaining -= size as u64;
        }
        let actual = digest.finalize();
        if actual.as_slice() != &footer[24..] {
            return Err("ONEFILE_PAYLOAD_HASH_MISMATCH".into());
        }
        Ok(Self {
            file,
            offset,
            length,
            digest: format!("{actual:x}"),
        })
    }
    fn reader(&mut self) -> SliceReader<'_> {
        SliceReader {
            file: &mut self.file,
            start: self.offset,
            length: self.length,
            position: 0,
        }
    }
}

struct SliceReader<'a> {
    file: &'a mut File,
    start: u64,
    length: u64,
    position: u64,
}
impl Read for SliceReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let count = output.len().min((self.length - self.position) as usize);
        self.file
            .seek(SeekFrom::Start(self.start + self.position))?;
        let read = self.file.read(&mut output[..count])?;
        self.position += read as u64;
        Ok(read)
    }
}
impl Seek for SliceReader<'_> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(n) => n as i128,
            SeekFrom::Current(n) => self.position as i128 + n as i128,
            SeekFrom::End(n) => self.length as i128 + n as i128,
        };
        if position < 0 || position > self.length as i128 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Seek outside embedded ZIP",
            ));
        }
        self.position = position as u64;
        Ok(self.position)
    }
}

#[derive(Deserialize)]
struct ManifestEntry {
    path: String,
    size: u64,
    sha256: String,
}
#[derive(Deserialize)]
struct Manifest {
    schema: String,
    version: String,
    files: Vec<ManifestEntry>,
}
#[derive(Serialize, Deserialize)]
struct CacheMarker {
    schema: String,
    payload_sha256: String,
    version: String,
    unpacked_bytes: u64,
}

pub fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':', '<', '>', '"', '|', '?', '*'])
        && path.split('/').all(|part| {
            let device = part
                .split('.')
                .next()
                .unwrap_or("")
                .trim_end()
                .to_uppercase();
            let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&device.as_str())
                || ["COM", "LPT"].iter().any(|prefix| {
                    device.strip_prefix(prefix).is_some_and(|n| {
                        ["1", "2", "3", "4", "5", "6", "7", "8", "9", "¹", "²", "³"].contains(&n)
                    })
                });
            !reserved
                && !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with([' ', '.'])
                && !part.chars().any(char::is_control)
        })
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

fn linked(path: &Path) -> Result<bool> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        Ok(metadata.file_attributes() & 0x400 != 0)
    }
    #[cfg(not(windows))]
    {
        Ok(metadata.file_type().is_symlink())
    }
}

fn walk_regular(root: &Path) -> Result<Vec<(String, u64)>> {
    fn walk(
        root: &Path,
        current: &Path,
        output: &mut Vec<(String, u64)>,
        depth: usize,
    ) -> Result<()> {
        if depth > 64 || linked(current)? {
            return Err("ONEFILE_CACHE_LINK_OR_DEPTH_INVALID".into());
        }
        for entry in fs::read_dir(current).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let path = entry.path();
            if linked(&path)? {
                return Err("ONEFILE_CACHE_LINK_INVALID".into());
            }
            let kind = entry.file_type().map_err(io_error)?;
            if kind.is_dir() {
                walk(root, &path, output, depth + 1)?;
            } else if kind.is_file() {
                let name = path
                    .strip_prefix(root)
                    .map_err(io_error)?
                    .to_string_lossy()
                    .replace('\\', "/");
                output.push((name, entry.metadata().map_err(io_error)?.len()));
                if output.len() > MAX_FILES + 4 {
                    return Err("ONEFILE_CACHE_FILE_LIMIT".into());
                }
            } else {
                return Err("ONEFILE_CACHE_FILE_INVALID".into());
            }
        }
        Ok(())
    }
    let mut output = Vec::new();
    walk(root, root, &mut output, 0)?;
    Ok(output)
}

fn file_hash(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(io_error)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn verify_runtime(root: &Path) -> Result<Manifest> {
    let files = walk_regular(root)?;
    let manifest = root.join("build-manifest.json");
    if fs::metadata(&manifest).map_err(io_error)?.len() > MAX_MANIFEST_BYTES {
        return Err("ONEFILE_MANIFEST_TOO_LARGE".into());
    }
    let value: Manifest =
        serde_json::from_slice(&fs::read(manifest).map_err(io_error)?).map_err(io_error)?;
    if value.schema != "nioh3-tauri-manifest/v1"
        || value.files.is_empty()
        || value.files.len() > MAX_FILES
    {
        return Err("ONEFILE_MANIFEST_INVALID".into());
    }
    let mut names = HashSet::new();
    let mut total = 0u64;
    for entry in &value.files {
        if !safe_relative(&entry.path)
            || !names.insert(entry.path.to_lowercase())
            || entry.sha256.len() != 64
            || !entry.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("ONEFILE_MANIFEST_PATH_INVALID".into());
        }
        let path = root.join(&entry.path);
        let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
        if !metadata.is_file()
            || metadata.len() != entry.size
            || file_hash(&path)? != entry.sha256.to_lowercase()
        {
            return Err(format!("ONEFILE_PACKAGE_FILE_MISMATCH: {}", entry.path));
        }
        total = total
            .checked_add(entry.size)
            .ok_or("ONEFILE_EXPANSION_LIMIT")?;
        if total > MAX_UNPACKED_BYTES {
            return Err("ONEFILE_EXPANSION_LIMIT".into());
        }
    }
    for required in [
        ENTRY,
        STUB,
        "worker/nioh3-search-worker.exe",
        "worker/nioh3-protected-worker.exe",
        "packages/contracts/request.schema.json",
        "packages/contracts/response.schema.json",
        "packages/contracts/protected-request.schema.json",
        "packages/contracts/protected-response.schema.json",
    ] {
        if !names.contains(&required.to_lowercase()) {
            return Err(format!("ONEFILE_PACKAGE_FILE_UNLISTED: {required}"));
        }
    }
    for (name, _) in files {
        if name != "build-manifest.json"
            && name != MARKER
            && name != STAGING_MARKER
            && name != LEASE
            && !names.contains(&name.to_lowercase())
        {
            return Err(format!("ONEFILE_CACHE_UNEXPECTED_FILE: {name}"));
        }
    }
    Ok(value)
}

struct ArchiveInfo {
    total: u64,
    manifest_digest: String,
    names: HashSet<String>,
}

fn inspect_archive<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> Result<ArchiveInfo> {
    if archive.is_empty() || archive.len() > MAX_FILES {
        return Err("ONEFILE_ZIP_FILE_LIMIT".into());
    }
    let mut names = HashMap::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(io_error)?;
        let name = entry.name().strip_suffix('/').unwrap_or(entry.name());
        let mode = entry.unix_mode().unwrap_or(0) & 0xf000;
        if !safe_relative(name)
            || name.split('/').count() > 64
            || [MARKER, STAGING_MARKER, LEASE].contains(&name.to_lowercase().as_str())
            || ![0, 0x4000, 0x8000].contains(&mode)
            || (mode == 0x4000 && !entry.is_dir())
            || (mode == 0x8000 && entry.is_dir())
            || names.insert(name.to_lowercase(), entry.is_dir()).is_some()
        {
            return Err(format!("ONEFILE_ZIP_PATH_INVALID: {name}"));
        }
        total = total
            .checked_add(entry.size())
            .ok_or("ONEFILE_EXPANSION_LIMIT")?;
        if total > MAX_UNPACKED_BYTES || entry.size() > MAX_ZIP_BYTES {
            return Err("ONEFILE_EXPANSION_LIMIT".into());
        }
    }
    // Refuse file/directory aliases before creating any ZIP-derived file.
    for name in names.keys() {
        let mut parent = name.as_str();
        while let Some((prefix, _)) = parent.rsplit_once('/') {
            if names.get(prefix) == Some(&false) {
                return Err("ONEFILE_ZIP_PARENT_IS_FILE".into());
            }
            parent = prefix;
        }
    }
    let mut manifest = archive.by_name("build-manifest.json").map_err(io_error)?;
    if manifest.size() > MAX_MANIFEST_BYTES {
        return Err("ONEFILE_MANIFEST_TOO_LARGE".into());
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut manifest)
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("ONEFILE_MANIFEST_TOO_LARGE".into());
    }
    Ok(ArchiveInfo {
        total,
        manifest_digest: sha256(&bytes),
        names: names.into_keys().collect(),
    })
}

fn extract(payload: &mut Payload, target: &Path) -> Result<u64> {
    let mut archive = zip::ZipArchive::new(payload.reader()).map_err(io_error)?;
    let info = inspect_archive(&mut archive)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(io_error)?;
        let path = target.join(entry.name());
        if entry.is_dir() {
            fs::create_dir_all(path).map_err(io_error)?;
            continue;
        }
        fs::create_dir_all(path.parent().ok_or("ONEFILE_ZIP_PARENT_MISSING")?).map_err(io_error)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(io_error)?;
        let expected = entry.size();
        let copied = io::copy(&mut Read::by_ref(&mut entry).take(expected + 1), &mut file)
            .map_err(io_error)?;
        if copied != expected {
            return Err("ONEFILE_ZIP_SIZE_MISMATCH".into());
        }
        file.sync_all().map_err(io_error)?;
    }
    verify_runtime(target)?;
    Ok(info.total)
}

fn open_lock(path: &Path) -> Result<File> {
    if path.exists() && linked(path)? {
        return Err("ONEFILE_LOCK_LINK_INVALID".into());
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(io_error)
}
fn exclusive(file: &File, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(()),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(25))
            }
            Err(TryLockError::WouldBlock) => {
                return Err("ONEFILE_CACHE_BUSY: another launch is preparing the runtime".into())
            }
            Err(TryLockError::Error(error)) => return Err(io_error(error)),
        }
    }
}

fn ensure_cache_root(root: &Path) -> Result<PathBuf> {
    fs::create_dir_all(root).map_err(io_error)?;
    if linked(root)? {
        return Err("ONEFILE_CACHE_ROOT_LINK_INVALID".into());
    }
    root.canonicalize().map_err(io_error)
}
fn child_path(parent: &Path, child: &Path) -> Result<()> {
    if linked(child)? || child.canonicalize().map_err(io_error)?.parent() != Some(parent) {
        return Err("ONEFILE_CLEANUP_PATH_INVALID".into());
    }
    Ok(())
}
fn read_marker(directory: &Path, name: &str) -> Result<CacheMarker> {
    let path = directory.join(name);
    if linked(&path)? || fs::metadata(&path).map_err(io_error)?.len() > 4096 {
        return Err("ONEFILE_CACHE_MARKER_INVALID".into());
    }
    let marker: CacheMarker =
        serde_json::from_slice(&fs::read(path).map_err(io_error)?).map_err(io_error)?;
    if marker.schema != "nioh3-onefile-cache/v1"
        || marker.payload_sha256.len() != 64
        || !marker.payload_sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("ONEFILE_CACHE_MARKER_INVALID".into());
    }
    Ok(marker)
}
fn write_marker(path: &Path, marker: &CacheMarker) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(io_error)?;
    file.write_all(&serde_json::to_vec(marker).map_err(io_error)?)
        .map_err(io_error)?;
    file.sync_all().map_err(io_error)
}

/// Shared locks belong to launcher processes, never to app profile files.
pub struct RuntimeLease {
    pub directory: PathBuf,
    pub digest: String,
    _lease: File,
}
impl RuntimeLease {
    pub fn prepare(cache_root: &Path, payload: &mut Payload) -> Result<Self> {
        let info = inspect_archive(&mut zip::ZipArchive::new(payload.reader()).map_err(io_error)?)?;
        let root = ensure_cache_root(cache_root)?;
        let global = open_lock(&root.join(".cache.lock"))?;
        exclusive(&global, Duration::from_secs(90))?;
        prune_staging(&root);
        let directory = root.join(&payload.digest);
        if directory.exists() {
            child_path(&root, &directory)?;
            let marker = read_marker(&directory, MARKER)?;
            if marker.payload_sha256 != payload.digest {
                return Err("ONEFILE_CACHE_IDENTITY_MISMATCH".into());
            }
            // Reusing a cache is never permission to trust changed executable bytes.
            let valid = file_hash(&directory.join("build-manifest.json"))
                .is_ok_and(|digest| digest == info.manifest_digest)
                && verify_runtime(&directory).is_ok();
            if !valid {
                let lease = open_lock(&directory.join(LEASE))?;
                if lease.try_lock().is_err() || runtime_process_present(&directory) {
                    return Err(
                        "ONEFILE_ACTIVE_CACHE_DAMAGED: close other app windows before retrying"
                            .into(),
                    );
                }
                let files = walk_regular(&directory)?;
                if files.iter().any(|(name, _)| {
                    name != MARKER && name != LEASE && !info.names.contains(&name.to_lowercase())
                }) {
                    return Err(format!(
                        "ONEFILE_CACHE_UNEXPECTED_FILES: {}",
                        directory.display()
                    ));
                }
                drop(lease);
                fs::remove_dir_all(&directory).map_err(io_error)?;
            }
        }
        if !directory.exists() {
            let stage = root.join(format!(
                ".staging-{}-{}",
                payload.digest,
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&stage).map_err(io_error)?;
            let marker = CacheMarker {
                schema: "nioh3-onefile-cache/v1".into(),
                payload_sha256: payload.digest.clone(),
                version: String::new(),
                unpacked_bytes: 0,
            };
            let prepared = (|| {
                write_marker(&stage.join(STAGING_MARKER), &marker)?;
                let unpacked_bytes = extract(payload, &stage)?;
                let version = verify_runtime(&stage)?.version;
                write_marker(
                    &stage.join(MARKER),
                    &CacheMarker {
                        version,
                        unpacked_bytes,
                        ..marker
                    },
                )?;
                fs::remove_file(stage.join(STAGING_MARKER)).map_err(io_error)?;
                fs::rename(&stage, &directory).map_err(io_error)
            })();
            if let Err(error) = prepared {
                if child_path(&root, &stage).is_ok() && walk_regular(&stage).is_ok() {
                    let _ = fs::remove_dir_all(&stage);
                }
                return Err(error);
            }
        }
        let lease = open_lock(&directory.join(LEASE))?;
        // Touch access time without changing an active lease's bytes or lock state.
        lease
            .set_times(fs::FileTimes::new().set_modified(SystemTime::now()))
            .map_err(io_error)?;
        lease.lock_shared().map_err(io_error)?;
        prune_locked(&root, &payload.digest);
        drop(global);
        Ok(Self {
            directory,
            digest: payload.digest.clone(),
            _lease: lease,
        })
    }
}

fn prune_staging(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten().take(MAX_FILES) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if !name.starts_with(".staging-") || child_path(root, &path).is_err() {
            continue;
        }
        let Ok(marker) = read_marker(&path, STAGING_MARKER) else {
            continue;
        };
        if !name.starts_with(&format!(".staging-{}-", marker.payload_sha256)) {
            continue;
        }
        if walk_regular(&path).is_ok() {
            let _ = fs::remove_dir_all(path);
        }
    }
}

fn prune_locked(root: &Path, current: &str) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let mut caches = Vec::new();
    for entry in entries.flatten().take(MAX_FILES) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.len() != 64 || !name.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let path = entry.path();
        if child_path(root, &path).is_err() {
            continue;
        }
        let Ok(marker) = read_marker(&path, MARKER) else {
            continue;
        };
        if marker.payload_sha256 != name {
            continue;
        }
        let used = fs::metadata(path.join(LEASE))
            .and_then(|m| m.modified())
            .unwrap_or(UNIX_EPOCH);
        caches.push((name, path, used, marker.unpacked_bytes));
    }
    caches.sort_by(|a, b| b.2.cmp(&a.2));
    let mut kept = 0;
    let mut bytes = 0u64;
    for (name, path, _, size) in caches {
        if name == current || (kept < MAX_CACHES && bytes.saturating_add(size) <= MAX_CACHE_BYTES) {
            kept += 1;
            bytes = bytes.saturating_add(size);
            continue;
        }
        let Ok(lease) = open_lock(&path.join(LEASE)) else {
            continue;
        };
        if lease.try_lock().is_err() {
            continue;
        } // A different live launcher owns this cache.
        if runtime_process_present(&path) {
            continue;
        }
        if verify_runtime(&path).is_err() {
            continue;
        } // Never delete extra user files or changed trees.
        drop(lease);
        // All launchers acquire the global lock before their shared lease.
        // No new runtime user can enter between the exclusive probe and deletion.
        let _ = fs::remove_dir_all(path);
    }
}

/// A protected worker can outlive its UI/launcher. Retain its entire runtime,
/// rather than discovering an open executable halfway through recursive removal.
#[cfg(windows)]
fn runtime_process_present(directory: &Path) -> bool {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    };
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return true;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut present = false;
        let mut available = Process32FirstW(snapshot, &mut entry);
        while available != 0 {
            let end = entry
                .szExeFile
                .iter()
                .position(|v| *v == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]).to_lowercase();
            if [
                "nioh3studio.exe",
                "nioh3-search-worker.exe",
                "nioh3-protected-worker.exe",
            ]
            .contains(&name.as_str())
            {
                let process =
                    OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID);
                if process.is_null() {
                    present = true;
                    break;
                }
                let mut text = vec![0u16; 32768];
                let mut size = text.len() as u32;
                let read = QueryFullProcessImageNameW(process, 0, text.as_mut_ptr(), &mut size);
                CloseHandle(process);
                if read == 0 {
                    present = true;
                    break;
                }
                let image = PathBuf::from(String::from_utf16_lossy(&text[..size as usize]));
                match image.canonicalize() {
                    Ok(path) if path.starts_with(directory) => {
                        present = true;
                        break;
                    }
                    Err(_) => {
                        present = true;
                        break;
                    }
                    _ => {}
                }
            }
            available = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        present
    }
}
#[cfg(not(windows))]
fn runtime_process_present(_directory: &Path) -> bool {
    false
}

pub fn default_cache_root() -> Result<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is not available")?;
    Ok(PathBuf::from(local).join("Nioh3Studio/onefile"))
}

pub fn launch(args: impl IntoIterator<Item = OsString>) -> Result<i32> {
    let outer = std::env::current_exe()
        .map_err(io_error)?
        .canonicalize()
        .map_err(io_error)?;
    let mut payload = Payload::open(&outer)?;
    let runtime = RuntimeLease::prepare(&default_cache_root()?, &mut payload)?;
    drop(payload); // The updater may replace the outer EXE after this launcher exits.
    let status = Command::new(runtime.directory.join(ENTRY))
        .args(args)
        .current_dir(&runtime.directory)
        .env("NIOH3_ONEFILE_EXE", &outer)
        .env("NIOH3_ONEFILE_PID", std::process::id().to_string())
        .env("NIOH3_ONEFILE_PAYLOAD_SHA256", &runtime.digest)
        .spawn()
        .map_err(|e| {
            format!(
                "Unable to launch {}: {e}",
                runtime.directory.join(ENTRY).display()
            )
        })?
        .wait()
        .map_err(io_error)?;
    // Runtime reuse is bounded by pruning on future launches, without touching
    // profile state or deleting dependencies that a protected worker still uses.
    drop(runtime);
    Ok(status.code().unwrap_or(1))
}

pub fn log_launch_failure(message: &str) -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Nioh3Studio/logs");
    fs::create_dir_all(&root).ok()?;
    let path = root.join("launcher.log");
    if path.exists() && linked(&path).ok()? {
        return None;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;
    if file.metadata().ok()?.len() > 256 * 1024 {
        file.set_len(0).ok()?;
    }
    let text: String = message.chars().take(8192).collect();
    writeln!(
        file,
        "unix_seconds={} launcher={} {text}",
        timestamp(),
        env!("CARGO_PKG_VERSION")
    )
    .ok()?;
    Some(path)
}

#[cfg(test)]
mod tests;
