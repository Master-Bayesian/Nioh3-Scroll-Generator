//! The installed game executable's identity, resolved once per host session.
//!
//! A production launch must name the exact installed game build because the
//! worker graph selects its generation resources from that identity. The value
//! is therefore never a build-time constant, an environment guess, or a
//! hardcoded fallback: it is read from the real installed `Nioh3.exe`.
//!
//! Discovery is deliberately bounded. It looks only at Steam roots that can be
//! named without walking a filesystem - `ProgramFiles(x86)`, the two shipped
//! default locations, `HKCU\Software\Valve\Steam`, plus any library the Steam
//! root's own `libraryfolders.vdf` declares - and it never descends into
//! directories. Anything other than exactly one readable, four-part-versioned
//! executable fails closed with a code and a Steam-side action a user can take,
//! rather than letting a worker start against an identity nobody verified. No
//! refusal names an environment override, because this host has none.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// One validated Windows file version: exactly four `u16` components.
///
/// The type is the parser's only constructor, so a value that exists is already
/// known to be four in-range components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameFileVersion(u16, u16, u16, u16);

impl GameFileVersion {
    /// Parse the exact `a.b.c.d` spelling the worker binaries require.
    ///
    /// Exactly four decimal components, each in `0..=65535`, and no surrounding
    /// whitespace. That is the grammar `parse_game_file_version` applies in
    /// `crates/nioh3-worker/src/main.rs` and `crates/nioh3-protected/src/main.rs`:
    /// split on `.`, four parts, each parsed with `u16::from_str`, which reads no
    /// space. The workers additionally read a leading `+` on a component, a
    /// spelling this host never produces, so every value this host accepts is one
    /// both workers accept for its spelling.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn parse(raw: &str) -> Result<Self, String> {
        let parts: Vec<&str> = raw.split('.').collect();
        if parts.len() != 4 {
            return Err(format!(
                "GAME_VERSION_MALFORMED: expected four components such as 2.0.2.0, found {raw:?}"
            ));
        }
        let mut numbers = [0u16; 4];
        for (index, part) in parts.iter().enumerate() {
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(format!(
                    "GAME_VERSION_MALFORMED: component {part:?} is not a number in {raw:?}"
                ));
            }
            numbers[index] = part.parse::<u16>().map_err(|_| {
                format!("GAME_VERSION_MALFORMED: component {part:?} exceeds 65535 in {raw:?}")
            })?;
        }
        Ok(GameFileVersion(
            numbers[0], numbers[1], numbers[2], numbers[3],
        ))
    }

    /// The dotted spelling the worker argv carries.
    pub fn dotted(&self) -> String {
        format!("{}.{}.{}.{}", self.0, self.1, self.2, self.3)
    }
}

impl std::fmt::Display for GameFileVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.dotted())
    }
}

/// How the host reads a four-part file version from one executable path.
///
/// The host is generic over this trait so tests inject a deterministic reader
/// instead of depending on a real installed game. Production uses
/// [`WindowsFileVersionReader`].
pub trait FileVersionReader: Send + Sync {
    fn read(&self, executable: &Path) -> Result<GameFileVersion, String>;
}

/// The shipped reader: the Windows version resource of one executable.
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsFileVersionReader;

impl FileVersionReader for WindowsFileVersionReader {
    fn read(&self, executable: &Path) -> Result<GameFileVersion, String> {
        read_file_version(executable)
    }
}

#[cfg(windows)]
fn read_file_version(executable: &Path) -> Result<GameFileVersion, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };

    let wide: Vec<u16> = executable
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut ignored: u32 = 0;
    // SAFETY: `wide` is a NUL-terminated UTF-16 buffer that outlives the call,
    // and the out-parameter is a stack local.
    let size = unsafe { GetFileVersionInfoSizeW(wide.as_ptr(), &mut ignored) };
    if size == 0 {
        return Err(format!(
            "GAME_VERSION_UNREADABLE: no version resource on {}",
            executable.display()
        ));
    }
    let mut buffer = vec![0u8; size as usize];
    // SAFETY: the buffer is exactly `size` bytes as the previous call reported,
    // and the path buffer is the same NUL-terminated one.
    let loaded = unsafe {
        GetFileVersionInfoW(
            wide.as_ptr(),
            0,
            size,
            buffer.as_mut_ptr().cast::<std::ffi::c_void>(),
        )
    };
    if loaded == 0 {
        return Err(format!(
            "GAME_VERSION_UNREADABLE: cannot read the version resource of {}",
            executable.display()
        ));
    }
    let mut value: *mut std::ffi::c_void = std::ptr::null_mut();
    let mut length: u32 = 0;
    // SAFETY: the buffer still holds the resource `GetFileVersionInfoW` filled,
    // the sub-block is the fixed-info root, and both out-parameters are stack
    // locals. `"\".encode_utf16()` is the NUL-terminated `\` sub-block name.
    let queried = unsafe {
        VerQueryValueW(
            buffer.as_ptr().cast::<std::ffi::c_void>(),
            [b'\\' as u16, 0].as_ptr(),
            &mut value,
            &mut length,
        )
    };
    // A fixed-info block is 52 bytes; anything smaller is not the structure the
    // casts below assume.
    if queried == 0 || value.is_null() || length < 52 {
        return Err(format!(
            "GAME_VERSION_UNREADABLE: malformed version resource on {}",
            executable.display()
        ));
    }
    // SAFETY: `VerQueryValueW` returned a non-null pointer to at least 52 bytes
    // of fixed file information, whose first two `u32`s hold the file version
    // as two big-endian halves. Reading them as unaligned `u32` avoids assuming
    // any alignment the API does not promise.
    let (ms, ls) = unsafe {
        let base = value.cast::<u8>();
        let read = |offset: usize| std::ptr::read_unaligned(base.add(offset).cast::<u32>());
        (read(8), read(12))
    };
    Ok(GameFileVersion(
        (ms >> 16) as u16,
        (ms & 0xFFFF) as u16,
        (ls >> 16) as u16,
        (ls & 0xFFFF) as u16,
    ))
}

#[cfg(not(windows))]
fn read_file_version(executable: &Path) -> Result<GameFileVersion, String> {
    Err(format!(
        "GAME_VERSION_UNSUPPORTED_PLATFORM: cannot read a Windows file version of {}",
        executable.display()
    ))
}

/// The Steam roots this host may look in without walking a filesystem.
///
/// Every entry is either an environment-named install, one of the two shipped
/// default locations, or the path Steam itself records for the current user.
/// Missing entries are skipped; nothing here enumerates a directory.
/// Whether two library roots name the same directory for deduplication.
///
/// Windows path identity is case-insensitive, but `canonicalize` only reports
/// the on-disk spelling of a directory that is present: when a root does not
/// exist yet (a temp fixture, or a machine that lacks the drive) the same
/// library supplied as `D:\SteamLibrary` and `d:\steamlibrary` used to read as
/// two. The canonical form is still tried first - so a real junction, an 8.3
/// name, or a case-only difference on a present directory folds exactly as
/// before - and only the unavailable case falls back to a case-insensitive
/// comparison of the whole path, prefix included. Nothing is stripped, and
/// non-Windows builds keep exact comparison.
fn same_library_directory(left: &Path, right: &Path) -> bool {
    let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
    let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
    if left == right {
        return true;
    }
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn steam_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let from_env = |key: &str| {
        let value = std::env::var(key).ok()?;
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(PathBuf::from(trimmed))
        }
    };
    if let Some(program_files) = from_env("ProgramFiles(x86)") {
        roots.push(program_files.join("Steam"));
    }
    roots.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    roots.push(PathBuf::from(r"D:\Steam"));
    if let Some(steam_path) = steam_registry_path() {
        roots.push(steam_path);
    }
    // The same install can be named several ways - the shipped default and the
    // path Steam records for this user differ in separator and letter case - so
    // candidates are deduplicated by the directory they actually resolve to.
    // Otherwise one install would read as an ambiguous pair and no launch could
    // establish an identity.
    let mut unique: Vec<PathBuf> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    for root in roots {
        if seen
            .iter()
            .any(|candidate| same_library_directory(candidate, &root))
        {
            continue;
        }
        seen.push(root.clone());
        unique.push(root);
    }
    unique
}

/// `HKCU\Software\Valve\Steam`'s `SteamPath`, when this user has one.
#[cfg(windows)]
fn steam_registry_path() -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_SZ,
    };

    let subkey: Vec<u16> = "Software\\Valve\\Steam"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let value: Vec<u16> = "SteamPath"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: both name buffers are NUL-terminated and outlive the call, and
    // the out-parameter is a stack local.
    let opened =
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut key) };
    if opened != ERROR_SUCCESS {
        return None;
    }
    let mut kind: u32 = 0;
    let mut bytes: u32 = 0;
    // A first probe asks for the size; `ERROR_MORE_DATA` is the expected answer
    // for a string value and is not a failure here.
    // SAFETY: `key` was just opened, the value name is NUL-terminated, and the
    // size out-parameters are stack locals.
    let probed = unsafe {
        RegQueryValueExW(
            key,
            value.as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            std::ptr::null_mut(),
            &mut bytes,
        )
    };
    if !(probed == ERROR_SUCCESS || probed == ERROR_MORE_DATA) || kind != REG_SZ || bytes < 4 {
        // SAFETY: `key` is a handle this function opened and has not closed.
        unsafe { RegCloseKey(key) };
        return None;
    }
    let mut text = vec![0u16; (bytes as usize).div_ceil(2)];
    // SAFETY: the buffer is exactly `bytes` rounded up to a whole number of
    // UTF-16 units, which is the size the probe reported.
    let read = unsafe {
        RegQueryValueExW(
            key,
            value.as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            text.as_mut_ptr().cast::<u8>(),
            &mut bytes,
        )
    };
    // SAFETY: `key` is a handle this function opened and has not closed.
    unsafe { RegCloseKey(key) };
    if read != ERROR_SUCCESS {
        return None;
    }
    let end = text
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(text.len());
    let path = PathBuf::from(OsString::from_wide(&text[..end]));
    let trimmed = path.as_os_str().to_string_lossy().trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

#[cfg(not(windows))]
fn steam_registry_path() -> Option<PathBuf> {
    None
}

/// Every library path one `libraryfolders.vdf` declares.
///
/// The file is Valve's own key-value text; only `"path" "<value>"` pairs are
/// read, and the parser does not descend into anything the file mentions.
fn declared_libraries(vdf: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for line in vdf.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix('"') else {
            continue;
        };
        let Some((key, after)) = rest.split_once('"') else {
            continue;
        };
        if !key.eq_ignore_ascii_case("path") {
            continue;
        }
        let mut chars = after.trim_start().chars();
        if chars.next() != Some('"') {
            continue;
        }
        let mut value = String::new();
        let mut escaped = false;
        for character in chars {
            if escaped {
                value.push(character);
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                break;
            } else {
                value.push(character);
            }
        }
        let value = value.trim().to_string();
        if !value.is_empty() {
            let path = PathBuf::from(value);
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    paths
}

/// Candidate Steam roots plus the libraries their `libraryfolders.vdf` declares.
fn library_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    // Deduplicate on the resolved directory, not the spelling: a library that
    // Steam's own `libraryfolders.vdf` names and a default root name the same
    // place differently must be one library, not two.
    let mut seen: Vec<PathBuf> = Vec::new();
    let push = |root: PathBuf, seen: &mut Vec<PathBuf>, roots: &mut Vec<PathBuf>| {
        if seen
            .iter()
            .any(|candidate| same_library_directory(candidate, &root))
        {
            return;
        }
        seen.push(root.clone());
        roots.push(root);
    };
    for steam_root in steam_roots() {
        let vdf = steam_root.join("steamapps").join("libraryfolders.vdf");
        if let Ok(text) = std::fs::read_to_string(&vdf) {
            for library in declared_libraries(&text) {
                push(library, &mut seen, &mut roots);
            }
        }
        push(steam_root, &mut seen, &mut roots);
    }
    roots
}

/// The one installed `Nioh3.exe` this host accepts, or a named refusal.
///
/// Exactly one existing candidate is required. Zero candidates means the game
/// was not found where Steam keeps it; more than one means the identity is
/// ambiguous and picking one would be a guess.
fn discover_game_executable() -> Result<PathBuf, String> {
    let candidates: Vec<PathBuf> = library_roots()
        .into_iter()
        .map(expected_game_executable)
        .collect();
    trustworthy_game_executable(&candidates)
}

/// The one exact path a library uses for the installed game executable.
///
/// Deriving the path is the whole discovery rule: the host never descends a
/// directory, so this is a lookup at a known location rather than a search.
fn expected_game_executable(library: PathBuf) -> PathBuf {
    library
        .join("steamapps")
        .join("common")
        .join("Nioh3")
        .join("Nioh3.exe")
}

/// The refusal a player reads when no install is where Steam keeps it.
///
/// The text names only the roots this host actually reads and the Steam action
/// that puts an install there. A packaged host has no executable override, so an
/// instruction to set an environment variable would be a dead end for whoever
/// reads the message.
const EXECUTABLE_NOT_FOUND: &str =
    "GAME_EXECUTABLE_NOT_FOUND: no installed Nioh3.exe under the Steam roots this host \
     checks (the shipped defaults plus the libraries this user's Steam records); install \
     the game through Steam, or add its library folder in Steam's settings so this \
     user's install is one this host can name";

/// The one trustworthy candidate of `candidates`, or a named refusal.
///
/// Exactly one existing candidate is required. Zero candidates means the game
/// was not found where Steam keeps it; more than one means the identity is
/// ambiguous and picking one would be a guess. Candidates are deduplicated by
/// the file they actually resolve to, so one install named several ways is one
/// install rather than an ambiguous pair.
fn trustworthy_game_executable(candidates: &[PathBuf]) -> Result<PathBuf, String> {
    // A named candidate that is present but is not a readable file is named as
    // unusable. That refusal is distinct from "not found": the player has the
    // game where Steam keeps it and something else is wrong, so reporting the
    // not-found text would send them to reinstall instead of to the real cause.
    let mut unusable: Vec<PathBuf> = Vec::new();
    let mut missing = 0usize;
    let mut found: Vec<PathBuf> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    for candidate in candidates {
        let exists = candidate.exists();
        if !candidate.is_file() {
            if exists {
                unusable.push(candidate.clone());
            } else {
                missing += 1;
            }
            continue;
        }
        // `is_file` alone accepts a link or a reparse point that resolves
        // somewhere unintended, so a candidate that cannot be canonicalized is
        // not trustworthy either.
        if candidate.canonicalize().is_err() {
            unusable.push(candidate.clone());
            continue;
        }
        let identity = candidate
            .canonicalize()
            .unwrap_or_else(|_| candidate.clone());
        if !seen.contains(&identity) {
            seen.push(identity);
            found.push(candidate.clone());
        }
    }
    if found.is_empty() && !unusable.is_empty() {
        return Err(format!(
            "GAME_EXECUTABLE_UNREADABLE: the Nioh3.exe this host found at {} is not a \
             readable file; repair that Steam install (in Steam, verify the integrity \
             of the game files) so the path Steam records holds the real executable",
            unusable[0].display()
        ));
    }
    if found.is_empty() && unusable.is_empty() && missing > 0 {
        return Err(EXECUTABLE_NOT_FOUND.into());
    }
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => Err(EXECUTABLE_NOT_FOUND.into()),
        count => Err(format!(
            "GAME_EXECUTABLE_AMBIGUOUS: {count} Nioh3.exe candidates under the known Steam \
             roots; this host will not guess which install the workers should bind to"
        )),
    }
}

/// Why a resolved installed version is unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameVersionError {
    pub code: &'static str,
    pub detail: String,
}

impl GameVersionError {
    fn new(code: &'static str, detail: String) -> Self {
        Self { code, detail }
    }

    /// The one-line, user-actionable spelling the host reports and logs.
    pub fn message(&self) -> String {
        format!("{}: {}", self.code, self.detail)
    }
}

/// Where one session's installed-version value comes from.
///
/// A packaged production host resolves it from the installed executable. The
/// development and test shape may name the executable explicitly; neither shape
/// is reachable from the packaged resolver.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameVersionSource {
    /// The explicitly named executable a development launch may use.
    pub development_executable: Option<PathBuf>,
}

impl GameVersionSource {
    /// The development/test source named by an explicit override, if any.
    ///
    /// This is the only place a caller-supplied value can replace discovery, and
    /// the packaged resolver never consults it: production reads the installed
    /// executable, so an environment value cannot pin a released build.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_development_executable(executable: Option<PathBuf>) -> Self {
        Self {
            development_executable: executable
                .filter(|path| !path.as_os_str().to_string_lossy().trim().is_empty()),
        }
    }
}

/// Resolve the installed version once, and answer from that cached value.
///
/// The first caller reads the executable and validates it; later callers get the
/// same value without touching the filesystem again, so one launch session can
/// pass an identical identity to every role it starts. A failure is cached too:
/// a host that could not establish the identity must not silently retry into a
/// different answer mid-session.
pub struct GameFileVersionSource<R: FileVersionReader> {
    reader: R,
    source: GameVersionSource,
    resolved: Mutex<Option<Result<GameFileVersion, GameVersionError>>>,
    /// How many times the reader ran. Test support only.
    #[cfg(test)]
    reads: std::sync::atomic::AtomicUsize,
}

impl<R: FileVersionReader> GameFileVersionSource<R> {
    pub fn new(reader: R, source: GameVersionSource) -> Self {
        Self {
            reader,
            source,
            resolved: Mutex::new(None),
            #[cfg(test)]
            reads: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// How many times this source has read an executable. Test support only.
    #[cfg(test)]
    pub fn reads_for_test(&self) -> usize {
        self.reads.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// The session's version, reading the executable at most once.
    pub fn resolve(&self) -> Result<GameFileVersion, GameVersionError> {
        let mut guard = self
            .resolved
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(cached) = guard.as_ref() {
            return cached.clone();
        }
        let value = self.detect();
        *guard = Some(value.clone());
        value
    }

    fn detect(&self) -> Result<GameFileVersion, GameVersionError> {
        let executable = match self.source.development_executable.as_ref() {
            Some(named) => named.clone(),
            None => discover_game_executable().map_err(|error| {
                let (code, detail) = split_code(&error);
                GameVersionError::new(code, detail)
            })?,
        };
        if !executable.is_file() {
            return Err(GameVersionError::new(
                "GAME_EXECUTABLE_UNREADABLE",
                format!("{0} is not a readable file", executable.display()),
            ));
        }
        #[cfg(test)]
        self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.reader.read(&executable).map_err(|error| {
            let (code, detail) = split_code(&error);
            GameVersionError::new(code, detail)
        })
    }
}

/// The declared library paths of one `libraryfolders.vdf`. Test support only.
#[cfg(test)]
pub fn declared_libraries_for_test(vdf: &str) -> Vec<PathBuf> {
    declared_libraries(vdf)
}

/// The exact expected candidate paths for one library list. Test support only.
#[cfg(test)]
pub fn derive_candidate_paths_for_test(libraries: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut candidates: Vec<PathBuf> = Vec::new();
    for library in libraries {
        if seen
            .iter()
            .any(|candidate| same_library_directory(candidate, library))
        {
            continue;
        }
        seen.push(library.clone());
        candidates.push(expected_game_executable(library.clone()));
    }
    candidates
}

/// The one trustworthy candidate of a caller-supplied list. Test support only.
#[cfg(test)]
pub fn trustworthy_game_executable_for_test(candidates: &[PathBuf]) -> Result<PathBuf, String> {
    trustworthy_game_executable(candidates)
}

/// Split `CODE: detail` into its parts, mapping any unrecognised code onto the
/// generic unavailable code so a caller always reports a stable identifier.
fn split_code(message: &str) -> (&'static str, String) {
    let (code, detail) = message.split_once(':').unwrap_or((message, ""));
    let known = match code {
        "GAME_EXECUTABLE_NOT_FOUND" => "GAME_EXECUTABLE_NOT_FOUND",
        "GAME_EXECUTABLE_AMBIGUOUS" => "GAME_EXECUTABLE_AMBIGUOUS",
        "GAME_EXECUTABLE_UNREADABLE" => "GAME_EXECUTABLE_UNREADABLE",
        "GAME_VERSION_MALFORMED" => "GAME_VERSION_MALFORMED",
        "GAME_VERSION_UNREADABLE" => "GAME_VERSION_UNREADABLE",
        "GAME_VERSION_UNSUPPORTED_PLATFORM" => "GAME_VERSION_UNSUPPORTED_PLATFORM",
        _ => "GAME_VERSION_UNAVAILABLE",
    };
    (known, detail.trim().to_string())
}
