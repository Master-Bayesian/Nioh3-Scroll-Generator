use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, path::Path, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin, Command},
    sync::{oneshot, Mutex},
};
use uuid::Uuid;

pub type Reply = Result<Value, String>;

/// The worker-backend manifest a staged Rust package carries.
///
/// `tools/stage_rust_workers.py` writes it and `tools/package_tauri.py` stages it
/// into `worker/worker-backend.json`. A packaged host must select the Rust
/// backend from this file rather than from a development environment flag, so a
/// released build can only run the worker graph that was actually built and
/// staged into it.
pub const WORKER_BACKEND_MANIFEST: &str = "worker/worker-backend.json";

/// The two roles one packaged manifest may map onto the staged binaries.
const PROTECTED_ROLES: [&str; 2] = ["save", "runtime"];
/// The refusal a protected host returns before starting a second job.
const HOST_BUSY: &str = "BUSY: protected operation is still running";
/// Up to three seconds of waiting for a short background job to finish.
const BUSY_RETRIES: u32 = 40;
const BUSY_RETRY_MILLIS: u64 = 75;

/// Strip the Windows extended-length prefix so a canonicalized path can be
/// compared against an unresolved package root.
pub(crate) fn normalize_path(path: &Path) -> std::path::PathBuf {
    let text = path.to_string_lossy();
    for prefix in ["\\\\?\\UNC\\", "\\\\?\\"] {
        if let Some(rest) = text.strip_prefix(prefix) {
            return if prefix == "\\\\?\\UNC\\" {
                std::path::PathBuf::from(format!("\\\\{rest}"))
            } else {
                std::path::PathBuf::from(rest)
            };
        }
    }
    std::path::PathBuf::from(text.into_owned())
}

/// Whether `candidate` is `root` itself or lives inside it.
///
/// Confinement is checked on the canonicalized parent directory because the
/// declared file may not exist yet while the directory is still the thing that
/// proves the path cannot escape the package.
fn is_confined(root: &Path, candidate: &Path) -> bool {
    let resolved_root = normalize_path(&root.canonicalize().unwrap_or_else(|_| root.to_path_buf()));
    let resolved = normalize_path(&candidate.canonicalize().unwrap_or_else(|_| {
        candidate
            .parent()
            .and_then(|parent| parent.canonicalize().ok())
            .map(|parent| parent.join(candidate.file_name().unwrap_or_default()))
            .unwrap_or_else(|| candidate.to_path_buf())
    }));
    resolved == resolved_root || resolved.starts_with(&resolved_root)
}

/// Which launch shape one Rust worker process was built for.
///
/// The two shapes pass different launch-mode acknowledgements and are validated
/// against different roots, so the mode is part of the resolved backend rather
/// than something a caller can choose at spawn time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustLaunchMode {
    /// `--dev-preview-only` / `--dev-protected-only`, working-tree roots, and
    /// explicit path overrides from the environment.
    Development,
    /// `--packaged-worker`, package-confined roots from the staged manifest.
    Packaged,
}

/// The worker one role launches. Search and the protected roles take different
/// argv, so the role's launch carries its own shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleLaunch {
    Search(RustSearchLaunch),
    Protected(RustProtectedLaunch),
}

impl RoleLaunch {
    const fn mode(&self) -> RustLaunchMode {
        match self {
            Self::Search(launch) => launch.mode,
            Self::Protected(launch) => launch.mode,
        }
    }
}

/// Everything the Rust read-only worker launch needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSearchLaunch {
    pub mode: RustLaunchMode,
    pub executable: std::path::PathBuf,
    pub data_root: std::path::PathBuf,
    pub contract_dir: std::path::PathBuf,
    pub accelerator: std::path::PathBuf,
    /// The exact installed game executable version the identity binds to, when
    /// the launch resolved one. `None` selects the explicitly non-production
    /// legacy identity, which only a development launch may do.
    pub game_file_version: Option<String>,
    /// Explicit, visibly non-production identity opt-in for development only.
    pub legacy_test_context: bool,
}

impl RustSearchLaunch {
    /// The exact argv the binary requires for its launch mode.
    ///
    /// The worker refuses to start without exactly one launch-mode acknowledgement and it
    /// loads the product data manifest, the shipped contracts and the
    /// accelerator identity from its own arguments.
    pub fn arguments(&self) -> Vec<String> {
        let mut arguments = vec![match self.mode {
            RustLaunchMode::Development => "--dev-preview-only",
            RustLaunchMode::Packaged => "--packaged-worker",
        }
        .to_string()];
        // Identity selection exactly as the binary validates it: an explicit
        // development opt-in, or the resolved installed version this launch was
        // pinned to.
        if self.legacy_test_context {
            arguments.push("--legacy-test-context".to_string());
        } else if let Some(version) = self.game_file_version.as_deref() {
            arguments.push("--game-file-version".to_string());
            arguments.push(version.to_string());
        }
        arguments.extend([
            "--data-root".to_string(),
            self.data_root.display().to_string(),
            "--contract-dir".to_string(),
            self.contract_dir.display().to_string(),
            "--accelerator".to_string(),
            self.accelerator.display().to_string(),
        ]);
        arguments
    }
}

/// The environment variables a development launch may use to select the Rust
/// read-only worker.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RustSearchEnv {
    pub executable: Option<String>,
    pub data_root: Option<String>,
    pub contract_dir: Option<String>,
    pub accelerator: Option<String>,
    pub game_file_version: Option<String>,
    pub legacy_test_context: bool,
}

/// Read the selection environment once per host lookup.
pub fn rust_search_env() -> RustSearchEnv {
    let read = |key: &str| {
        std::env::var(key)
            .ok()
            .filter(|value| !value.trim().is_empty())
    };
    RustSearchEnv {
        executable: read("NIOH3_RUST_SEARCH_WORKER"),
        data_root: read("NIOH3_RUST_SEARCH_DATA_ROOT"),
        contract_dir: read("NIOH3_RUST_SEARCH_CONTRACT_DIR"),
        accelerator: read("NIOH3_RUST_SEARCH_ACCELERATOR"),
        game_file_version: read("NIOH3_RUST_SEARCH_GAME_FILE_VERSION"),
        legacy_test_context: std::env::var("NIOH3_RUST_SEARCH_LEGACY_CONTEXT")
            .is_ok_and(|value| value.trim() == "1"),
    }
}

/// The staged worker graph one package declares, with every declared path
/// already resolved and confined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedBackendManifest {
    /// Repository-relative or absolute path the manifest declared for the role.
    pub roles: HashMap<String, String>,
    pub data_root: std::path::PathBuf,
    pub contract_dir: std::path::PathBuf,
    pub accelerator: std::path::PathBuf,
    pub binary_sha256: HashMap<String, String>,
}

/// Read and validate the staged worker-backend manifest.
///
/// The manifest is data to validate, never an oracle: the schema name, the role
/// set, every declared resource and binary path and the package confinement are
/// all re-derived here. A package that declares a backend it cannot run is a
/// named failure, so a staged package never falls back to another graph.
pub fn staged_backend_manifest(root: &Path) -> Result<Option<StagedBackendManifest>, String> {
    let path = root.join(WORKER_BACKEND_MANIFEST);
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("WORKER_BACKEND_UNREADABLE: {error}"))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| format!("WORKER_BACKEND_MALFORMED: {error}"))?;
    if value["schema"] != "nioh3-worker-backend/v1" {
        return Err("WORKER_BACKEND_SCHEMA_UNSUPPORTED".into());
    }
    if value["backend"] != "rust" {
        return Err(format!("WORKER_BACKEND_UNSUPPORTED: {}", value["backend"]));
    }
    let resolve = |raw: &str, label: &str| -> Result<std::path::PathBuf, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(format!("WORKER_BACKEND_MALFORMED: empty {label}"));
        }
        // The staging tool cannot know the installed payload directory, so it
        // declares package-confined paths with a `<runtime>` placeholder. The
        // packaged host is the thing that knows that root, so it substitutes it
        // here rather than trusting a path the manifest could not have resolved.
        //
        // The manifest declares POSIX-style separators and the packaged root is
        // canonical (`\\?\...`). A canonical path is never normalized by the
        // filesystem, so a mixed-separator path is looked up literally and every
        // declared resource reads as missing. The root's own form is preserved
        // (stripping `\\?\` would break long paths); only the manifest's
        // separators are converted.
        let text = trimmed
            .replace("<runtime>", &root.display().to_string())
            .replace('/', std::path::MAIN_SEPARATOR_STR);
        let text = text.as_str();
        let declared = if Path::new(text).is_absolute() {
            std::path::PathBuf::from(text)
        } else {
            root.join(text)
        };
        if !is_confined(root, &declared) {
            return Err(format!(
                "WORKER_BACKEND_ESCAPES_PACKAGE: {label} {}",
                declared.display()
            ));
        }
        Ok(declared)
    };

    let mut roles = HashMap::new();
    let mut binary_sha256 = HashMap::new();
    let mut data_root = None;
    let mut contract_dir = None;
    for role in ["offline_search", "save", "runtime"] {
        let invocation = &value["invocation"][role];
        if !invocation.is_object() {
            return Err(format!("WORKER_BACKEND_ROLE_MISSING: {role}"));
        }
        if invocation["mode"] != "packaged" {
            return Err(format!("WORKER_BACKEND_ROLE_NOT_PACKAGED: {role}"));
        }
        let binary = invocation["binary"]
            .as_str()
            .ok_or_else(|| format!("WORKER_BACKEND_MALFORMED: {role} has no binary"))?;
        // The manifest declares a bare file name; the packaged runtime places
        // both binaries directly under `worker/`, which is the only directory the
        // broker resolves for a packaged launch.
        let declared = resolve(&format!("worker/{binary}"), &format!("{role} binary"))?;
        if !declared.is_file() {
            return Err(format!(
                "RUST_WORKER_MISSING: declared {role} binary is absent: {}",
                declared.display()
            ));
        }
        if role == "offline_search" && binary != "nioh3-search-worker.exe" {
            return Err(format!("WORKER_BACKEND_ROLE_BINARY: {role} is {binary}"));
        }
        if PROTECTED_ROLES.contains(&role) && binary != "nioh3-protected-worker.exe" {
            return Err(format!("WORKER_BACKEND_ROLE_BINARY: {role} is {binary}"));
        }
        let argv = invocation["argv"]
            .as_array()
            .ok_or_else(|| format!("WORKER_BACKEND_MALFORMED: {role} has no argv"))?
            .iter()
            .map(|item| item.as_str().map(str::to_string))
            .collect::<Option<Vec<String>>>()
            .ok_or_else(|| format!("WORKER_BACKEND_MALFORMED: {role} argv is not text"))?;
        let flag_value = |flag: &str| -> Result<String, String> {
            let index = argv
                .iter()
                .position(|token| token == flag)
                .ok_or_else(|| format!("WORKER_BACKEND_MALFORMED: {role} argv lacks {flag}"))?;
            argv.get(index + 1)
                .cloned()
                .ok_or_else(|| format!("WORKER_BACKEND_MALFORMED: {role} {flag} has no value"))
        };
        let role_data_root = resolve(&flag_value("--data-root")?, "--data-root")?;
        let role_contract_dir = resolve(&flag_value("--contract-dir")?, "--contract-dir")?;
        if !role_data_root.is_dir() {
            return Err(format!(
                "WORKER_BACKEND_RESOURCE_MISSING: data root {}",
                role_data_root.display()
            ));
        }
        if !role_contract_dir.is_dir() {
            return Err(format!(
                "WORKER_BACKEND_RESOURCE_MISSING: contract dir {}",
                role_contract_dir.display()
            ));
        }
        match &data_root {
            Some(previous) if previous != &role_data_root => {
                return Err("WORKER_BACKEND_ROOTS_DISAGREE: --data-root".into())
            }
            _ => data_root = Some(role_data_root),
        }
        match &contract_dir {
            Some(previous) if previous != &role_contract_dir => {
                return Err("WORKER_BACKEND_ROOTS_DISAGREE: --contract-dir".into())
            }
            _ => contract_dir = Some(role_contract_dir),
        }
        roles.insert(role.to_string(), declared.display().to_string());
        let sha = value["binaries"]
            .as_array()
            .and_then(|entries| entries.iter().find(|entry| entry["packagedName"] == binary))
            .and_then(|entry| entry["sha256"].as_str())
            .unwrap_or("");
        if sha.len() != 64 || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("WORKER_BACKEND_MALFORMED: {role} has no sha256"));
        }
        binary_sha256.insert(role.to_string(), sha.to_string());
    }
    // The accelerator is the one declared resource a package may legitimately
    // omit: without it the worker still starts and fails closed per search route.
    // Resolve it the way the worker resolves it (`<application root>/bin`), so a
    // packaged launch passes the same explicit helper the development launch
    // passes and the two shapes cannot drift.
    let data_root = data_root.ok_or("WORKER_BACKEND_MALFORMED: no data root")?;
    let application_root = data_root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.to_path_buf());
    let accelerator = resolve(
        &application_root
            .join("bin")
            .join("nioh3_seed_accelerator.dll")
            .to_string_lossy(),
        "--accelerator",
    )?;
    Ok(Some(StagedBackendManifest {
        roles,
        data_root,
        contract_dir: contract_dir.ok_or("WORKER_BACKEND_MALFORMED: no contract dir")?,
        accelerator,
        binary_sha256,
    }))
}

/// Override the session's installed-version resolution. Test support only.
///
/// Packaged production always reads the real installed executable; this exists
/// so a focused test can drive the packaged resolver without a game install,
/// and it is compiled out of every non-test build.
#[cfg(test)]
pub fn set_packaged_game_file_version_for_test(value: Option<String>) -> Result<(), String> {
    let mut guard = test_game_file_version()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = value;
    Ok(())
}

/// Force the session's installed-version resolution to fail. Test support only.
///
/// The packaged resolver normally reads the real installed executable; this
/// lets a focused test drive the "no trustworthy identity on this host" shape
/// without a game install. It is compiled out of every non-test build.
#[cfg(test)]
pub fn set_packaged_game_file_version_error_for_test(value: Option<String>) -> Result<(), String> {
    let mut guard = test_game_file_version_error()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = value;
    Ok(())
}

#[cfg(test)]
static TEST_GAME_FILE_VERSION_ERROR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[cfg(test)]
fn test_game_file_version_error() -> &'static std::sync::Mutex<Option<String>> {
    &TEST_GAME_FILE_VERSION_ERROR
}

#[cfg(test)]
static TEST_GAME_FILE_VERSION: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[cfg(test)]
fn test_game_file_version() -> &'static std::sync::Mutex<Option<String>> {
    &TEST_GAME_FILE_VERSION
}

/// The installed game version one packaged host session binds its workers to.
#[cfg(not(test))]
static INSTALLED_VERSION_SOURCE: std::sync::OnceLock<
    Result<
        crate::game_version::GameFileVersionSource<crate::game_version::WindowsFileVersionReader>,
        String,
    >,
> = std::sync::OnceLock::new();

/// Bind the native-picked path at bootstrap. Choosing/resetting a path later
/// only changes next-launch configuration; existing roles keep their context.
pub fn initialize_game_install(data: &Path) {
    #[cfg(not(test))]
    INSTALLED_VERSION_SOURCE.get_or_init(|| {
        let executable = crate::game_install::selected(data)?;
        Ok(crate::game_version::GameFileVersionSource::new(
            crate::game_version::WindowsFileVersionReader,
            crate::game_version::GameVersionSource {
                development_executable: executable,
            },
        ))
    });
    #[cfg(test)]
    let _ = data;
}
/// The installed game version one packaged host session binds its workers to.
///
/// The value is read from the real installed executable, validated as exactly
/// four components, and cached for the life of the process, so every role this
/// session starts receives one identical identity. A host that cannot establish
/// it fails closed here - before a worker accepts a job - rather than passing a
/// default, a build-time constant, or an environment guess into production.
pub fn packaged_game_file_version() -> Result<String, String> {
    // A test build never reads this machine's real game install: that would make
    // the packaged resolvers depend on a developer's disk and let one test
    // observe another test's session. A test that needs a resolved session
    // installs one through the test hook, so an unbound session is unavailable
    // here exactly as an uninstalled game is unavailable in production.
    #[cfg(test)]
    {
        let forced_error = test_game_file_version_error()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if let Some(error) = forced_error {
            return Err(error);
        }
        let installed = test_game_file_version()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        match installed {
            Some(value) => Ok(value),
            None => Err(
                "GAME_VERSION_UNAVAILABLE: no packaged session version is installed for this \
                 test"
                    .to_string(),
            ),
        }
    }
    #[cfg(not(test))]
    {
        let source = INSTALLED_VERSION_SOURCE
            .get_or_init(|| {
                Ok(crate::game_version::GameFileVersionSource::new(
                    crate::game_version::WindowsFileVersionReader,
                    crate::game_version::GameVersionSource::default(),
                ))
            })
            .as_ref()
            .map_err(Clone::clone)?;
        let version = source.resolve().map_err(|error| error.message())?;
        // A worker exits before handshake for an unregistered version. Reject
        // through the same registry here so recovery keeps an actionable error
        // and never leaves a dead protected process in the broker.
        version.ensure_supported()?;
        Ok(version.dotted())
    }
}

/// The Rust launch one role resolves to from a validated staged manifest.
pub fn packaged_rust_launch(
    manifest: &StagedBackendManifest,
    role: &str,
    game_file_version: &str,
) -> Result<(std::path::PathBuf, RustSearchLaunch), String> {
    let declared = manifest
        .roles
        .get(role)
        .ok_or_else(|| format!("WORKER_BACKEND_ROLE_MISSING: {role}"))?;
    let executable = std::path::PathBuf::from(declared);
    if !executable.is_file() {
        return Err(format!(
            "RUST_WORKER_MISSING: declared {role} binary is absent: {}",
            executable.display()
        ));
    }
    let launch = RustSearchLaunch {
        mode: RustLaunchMode::Packaged,
        executable: executable.clone(),
        data_root: manifest.data_root.clone(),
        contract_dir: manifest.contract_dir.clone(),
        accelerator: manifest.accelerator.clone(),
        // A packaged launch always carries the session's resolved installed
        // version. The `None` shape exists only for development launches, and
        // the packaged resolvers below never produce it.
        game_file_version: Some(game_file_version.to_string()),
        legacy_test_context: false,
    };
    Ok((executable, launch))
}

/// Resolve the read-only search backend for one launch.
///
/// A package that carries a staged Rust manifest runs its declared Rust worker.
/// A package without one fails closed. In development
/// `NIOH3_RUST_SEARCH_WORKER` selects the Rust worker explicitly and the three
/// optional variables override its paths; the defaults are the working-tree
/// paths the binary needs.
pub fn search_backend(
    root: &Path,
    packaged: bool,
    env: &RustSearchEnv,
) -> Result<RustSearchLaunch, String> {
    if packaged {
        let manifest = staged_backend_manifest(root)?.ok_or(MISSING_MANIFEST)?;
        let version = packaged_game_file_version()?;
        let (_, launch) = packaged_rust_launch(&manifest, "offline_search", &version)?;
        return Ok(launch);
    }
    let Some(named) = env
        .executable
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err(
            "RUST_WORKER_MISSING: set NIOH3_RUST_SEARCH_WORKER to the built search worker".into(),
        );
    };
    let executable = std::path::PathBuf::from(named);
    if !executable.is_file() {
        return Err(format!(
            "RUST_WORKER_MISSING: NIOH3_RUST_SEARCH_WORKER does not name a file: {}",
            executable.display()
        ));
    }
    let path = |value: &Option<String>, fallback: &str| {
        value
            .as_deref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| root.join(fallback))
    };
    Ok(RustSearchLaunch {
        mode: RustLaunchMode::Development,
        executable,
        data_root: path(&env.data_root, "data"),
        contract_dir: path(&env.contract_dir, "packages/contracts"),
        accelerator: path(&env.accelerator, "bin/nioh3_seed_accelerator.dll"),
        game_file_version: env.game_file_version.clone(),
        legacy_test_context: env.legacy_test_context,
    })
}

const MISSING_MANIFEST: &str = "WORKER_BACKEND_MISSING: the package declares no Rust worker";

/// Everything the development Rust protected launch needs that is not already
/// supplied per role (the state root is the host's own data directory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustProtectedLaunch {
    pub mode: RustLaunchMode,
    pub executable: std::path::PathBuf,
    pub data_root: std::path::PathBuf,
    pub contract_dir: std::path::PathBuf,
    pub accelerator: std::path::PathBuf,
    /// The generation version selected for save or explicitly in development.
    /// Packaged runtime has no version until an operation resolves its target.
    pub game_file_version: Option<String>,
    /// Explicit, visibly non-production identity opt-in for development only.
    pub legacy_test_context: bool,
}

impl RustProtectedLaunch {
    /// The exact argv the binary requires for `role` and its launch mode.
    ///
    /// The state root is always injected explicitly. A packaged launch must pass
    /// it because the protected worker resolves user state from an argument (or
    /// the broker's `NIOH3_STATE_ROOT`), never from its own directory.
    pub fn arguments(&self, role: &str, state_root: &Path) -> Vec<String> {
        let mut arguments = vec!["--role".to_string(), role.to_string()];
        if self.mode == RustLaunchMode::Development {
            arguments.push("--dev-protected-only".to_string());
        }
        // Identity selection exactly as the binary validates it: an explicit
        // development opt-in, or the resolved installed version this launch was
        // pinned to.
        if self.legacy_test_context {
            arguments.push("--legacy-test-context".to_string());
        } else if let Some(version) = self.game_file_version.as_deref() {
            arguments.push("--game-file-version".to_string());
            arguments.push(version.to_string());
        }
        arguments.extend([
            "--state-root".to_string(),
            state_root.display().to_string(),
            "--data-root".to_string(),
            self.data_root.display().to_string(),
            "--contract-dir".to_string(),
            self.contract_dir.display().to_string(),
            "--accelerator".to_string(),
            self.accelerator.display().to_string(),
        ]);
        arguments
    }
}

/// The environment variables a development launch may use to select the Rust
/// protected worker.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RustProtectedEnv {
    pub executable: Option<String>,
    pub data_root: Option<String>,
    pub contract_dir: Option<String>,
    pub accelerator: Option<String>,
    pub game_file_version: Option<String>,
    pub legacy_test_context: bool,
}

/// Read the protected selection environment once per host lookup.
pub fn rust_protected_env() -> RustProtectedEnv {
    let read = |key: &str| {
        std::env::var(key)
            .ok()
            .filter(|value| !value.trim().is_empty())
    };
    RustProtectedEnv {
        executable: read("NIOH3_RUST_PROTECTED_WORKER"),
        data_root: read("NIOH3_RUST_PROTECTED_DATA_ROOT"),
        contract_dir: read("NIOH3_RUST_PROTECTED_CONTRACT_DIR"),
        accelerator: read("NIOH3_RUST_PROTECTED_ACCELERATOR"),
        game_file_version: read("NIOH3_RUST_PROTECTED_GAME_FILE_VERSION"),
        legacy_test_context: std::env::var("NIOH3_RUST_PROTECTED_LEGACY_CONTEXT")
            .is_ok_and(|value| value.trim() == "1"),
    }
}

/// Resolve the protected backend for one launch.
///
/// A package that carries a staged Rust manifest runs its declared Rust worker
/// for both protected roles; anything else keeps the shipped worker, so a stray
/// variable in a user's environment cannot change the released product.
pub fn protected_backend(
    root: &Path,
    packaged: bool,
    env: &RustProtectedEnv,
) -> Result<RustProtectedLaunch, String> {
    if packaged {
        let manifest = staged_backend_manifest(root)?.ok_or(MISSING_MANIFEST)?;
        let version = packaged_game_file_version()?;
        let (_, search) = packaged_rust_launch(&manifest, "save", &version)?;
        // Both protected roles must resolve to the same declared binary and the
        // same roots, or the package is not internally consistent.
        let (_, runtime) = packaged_rust_launch(&manifest, "runtime", &version)?;
        if runtime.executable != search.executable {
            return Err("WORKER_BACKEND_ROLE_BINARY: protected roles disagree".into());
        }
        return Ok(RustProtectedLaunch {
            mode: search.mode,
            executable: search.executable,
            data_root: search.data_root,
            contract_dir: search.contract_dir,
            accelerator: search.accelerator,
            game_file_version: search.game_file_version,
            legacy_test_context: search.legacy_test_context,
        });
    }
    let Some(named) = env
        .executable
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err("RUST_PROTECTED_WORKER_MISSING: set NIOH3_RUST_PROTECTED_WORKER to the built protected worker".into());
    };
    let executable = std::path::PathBuf::from(named);
    if !executable.is_file() {
        return Err(format!(
            "RUST_PROTECTED_WORKER_MISSING: NIOH3_RUST_PROTECTED_WORKER does not name a file: {}",
            executable.display()
        ));
    }
    let path = |value: &Option<String>, fallback: &str| {
        value
            .as_deref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| root.join(fallback))
    };
    Ok(RustProtectedLaunch {
        mode: RustLaunchMode::Development,
        executable,
        data_root: path(&env.data_root, "data"),
        contract_dir: path(&env.contract_dir, "packages/contracts"),
        accelerator: path(&env.accelerator, "bin/nioh3_seed_accelerator.dll"),
        game_file_version: env.game_file_version.clone(),
        legacy_test_context: env.legacy_test_context,
    })
}

/// Resolve only the requested protected role. A packaged runtime starts with
/// no generation context; operations resolve the running target and load only
/// their required resources. Save keeps the installed-version contract.
pub fn protected_backend_for_role(
    root: &Path,
    role: &str,
    packaged: bool,
    env: &RustProtectedEnv,
) -> Result<RustProtectedLaunch, String> {
    if !PROTECTED_ROLES.contains(&role) {
        return Err("INVALID_ROLE".into());
    }
    if role != "runtime" || !packaged {
        // Preserve explicit development version/legacy opt-ins and save's
        // existing resolver. Neither is an implicit production fallback.
        return protected_backend(root, packaged, env);
    }
    let manifest = staged_backend_manifest(root)?.ok_or(MISSING_MANIFEST)?;
    let executable = manifest
        .roles
        .get(role)
        .map(std::path::PathBuf::from)
        .ok_or_else(|| format!("WORKER_BACKEND_ROLE_MISSING: {role}"))?;
    // The broker uses this resolver directly, so actual launch admission also
    // verifies staged bytes rather than relying on the diagnostic resolver.
    verify_declared_binary(&manifest, role, &executable)?;
    Ok(RustProtectedLaunch {
        mode: RustLaunchMode::Packaged,
        executable,
        data_root: manifest.data_root,
        contract_dir: manifest.contract_dir,
        accelerator: manifest.accelerator,
        game_file_version: None,
        legacy_test_context: false,
    })
}

/// Development selectors must not veto the packaged argv or redirect its DLLs.
/// Keep the developer shell intact; remove only overrides in this child command.
pub(crate) fn isolate_packaged_worker_environment(command: &mut Command, packaged: bool) {
    if packaged {
        for name in [
            "NIOH3_RUST_SEARCH_WORKER",
            "NIOH3_RUST_PROTECTED_WORKER",
            "NIOH3_SEED_ACCELERATOR",
            "NIOH3_EFFECT_PREIMAGE_ACCELERATOR",
        ] {
            command.env_remove(name);
        }
    }
}

pub struct Worker {
    role: String,
    /// Which backend graph this launch resolved to.
    backend: String,
    /// Sha256 of the spawned binary, so support can tell which worker answered.
    binary_sha256: Option<String>,
    child: Mutex<Child>,
    input: Mutex<Option<ChildStdin>>,
    pending: Mutex<HashMap<String, oneshot::Sender<Reply>>>,
    lifecycle: std::sync::Mutex<Lifecycle>,
    request_schema: jsonschema::Validator,
    response_schema: jsonschema::Validator,
    contract_digest: String,
    identity: Mutex<Option<Value>>,
}

/// Where one worker process stands. `SafelyClosed` is final; a protected host
/// that is `Lost` may still own a write until its process has exited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Serving,
    Lost,
    SafelyClosed,
}

/// The executable plus argv one role launches with.
pub(crate) fn launch_command(
    role: &str,
    launch: &RoleLaunch,
    state_root: &Path,
) -> (std::path::PathBuf, Vec<String>) {
    match launch {
        RoleLaunch::Search(search) => (search.executable.clone(), search.arguments()),
        RoleLaunch::Protected(protected) => (
            protected.executable.clone(),
            protected.arguments(role, state_root),
        ),
    }
}

/// Which backend graph this launch resolved to, for diagnostics and support.
pub fn backend_identifier(launch: &RoleLaunch) -> &'static str {
    match launch.mode() {
        RustLaunchMode::Packaged => "rust-packaged",
        RustLaunchMode::Development => "rust-development",
    }
}

/// Sha256 of the spawned executable, or `None` when it cannot be read.
///
fn binary_digest(executable: &Path) -> Option<String> {
    let bytes = std::fs::read(executable).ok()?;
    Some(format!("{:x}", Sha256::digest(&bytes)))
}

/// The sha256 the staged manifest declares for one role's binary.
///
/// Acceptance-only: it exists so the packaged gate can compare the manifest's
/// declared identity with the bytes the resolver actually selected.
#[allow(dead_code)]
pub fn declared_binary_sha256<'a>(
    manifest: &'a StagedBackendManifest,
    role: &str,
) -> Result<&'a str, String> {
    manifest
        .binary_sha256
        .get(role)
        .map(String::as_str)
        .ok_or_else(|| format!("WORKER_BACKEND_MALFORMED: {role} has no sha256"))
}

/// The declared identity of one role's staged binary must match the bytes.
///
/// The manifest is a declaration, not an oracle: a package whose staged binary
/// was replaced after staging is refused by name instead of being launched,
/// because launching would otherwise run a binary nobody accepted.
pub fn verify_declared_binary(
    manifest: &StagedBackendManifest,
    role: &str,
    executable: &Path,
) -> Result<(), String> {
    let declared = declared_binary_sha256(manifest, role)?;
    let actual = binary_digest(executable).ok_or_else(|| {
        format!(
            "RUST_WORKER_MISSING: declared {role} binary cannot be read: {}",
            executable.display()
        )
    })?;
    if !actual.eq_ignore_ascii_case(declared) {
        return Err(format!(
            "RUST_WORKER_CHANGED: declared {role} sha256 {declared}, staged file is {actual}"
        ));
    }
    Ok(())
}

/// The launch one role resolves to.
pub fn role_launch(root: &Path, role: &str, packaged: bool) -> Result<RoleLaunch, String> {
    match role {
        "offline_search" => Ok(RoleLaunch::Search(search_backend(
            root,
            packaged,
            &rust_search_env(),
        )?)),
        "save" | "runtime" => Ok(RoleLaunch::Protected(protected_backend_for_role(
            root,
            role,
            packaged,
            &rust_protected_env(),
        )?)),
        _ => Err("INVALID_ROLE".into()),
    }
}

/// The exact command line a packaged host resolves for one role.
///
/// This is the single entry point the packaged acceptance drives, so the argv a
/// gate observes is the argv the host would spawn rather than a copy of the
/// staging tool's expectations.
#[allow(dead_code)]
pub fn resolve_role_launch(
    root: &Path,
    role: &str,
    packaged: bool,
    state_root: &Path,
) -> Result<(std::path::PathBuf, Vec<String>), String> {
    let launch = role_launch(root, role, packaged)?;
    // Runtime does not consult the installed version or borrow its generation data.
    if packaged && role != "runtime" {
        packaged_game_file_version()?;
    }
    let launch = launch_command(role, &launch, state_root);
    // Runtime's role resolver already verifies its binary before broker spawn.
    if packaged && role != "runtime" {
        if let Some(manifest) = staged_backend_manifest(root)? {
            verify_declared_binary(&manifest, role, &launch.0)?;
        }
    }
    Ok(launch)
}

impl Worker {
    /// A broken pipe alone is not permission to replace a protected owner.
    pub async fn can_replace(&self) -> bool {
        match self.lifecycle() {
            Lifecycle::Serving => false,
            Lifecycle::SafelyClosed => true,
            Lifecycle::Lost => self.child.lock().await.try_wait().ok().flatten().is_some(),
        }
    }
    fn lifecycle(&self) -> Lifecycle {
        *self.lifecycle.lock().expect("worker lifecycle lock")
    }
    fn lose(&self) {
        let mut lifecycle = self.lifecycle.lock().expect("worker lifecycle lock");
        if *lifecycle == Lifecycle::Serving {
            *lifecycle = Lifecycle::Lost;
        }
    }
    #[cfg(test)]
    pub async fn disconnect_for_test(&self) {
        self.fail("TEST_TRANSPORT_LOST").await;
    }

    /// The recorded exit status once this worker's process has exited.
    #[cfg(test)]
    pub async fn exit_status(&self) -> Option<std::process::ExitStatus> {
        self.child.lock().await.try_wait().ok().flatten()
    }

    /// Whether a transport failure may terminate this worker's process.
    ///
    /// Only the read-only search worker is replaceable by force: it owns no
    /// game or save state, so reaping it cannot abandon an owner. A protected
    /// save/runtime host owns live state, so it is only ever asked to finish
    /// cleanly; its process is never terminated on our side.
    fn may_force_kill(&self) -> bool {
        self.role == "offline_search"
    }

    pub async fn diagnostics(&self) -> Value {
        // A pending handshake holds identity across an await. Support export
        // must not wait for that handshake or for the failing worker itself.
        let context = self.identity.try_lock().ok().and_then(|guard| {
            guard
                .as_ref()
                .map(|v| v["context"]["context_digest"].clone())
        });
        let pending = self.pending.try_lock().ok().map(|value| value.len());
        let capabilities = self
            .identity
            .try_lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|v| v["capabilities"].clone()));
        json!({"role":self.role,"connection":match self.lifecycle(){Lifecycle::SafelyClosed=>"closed",
            Lifecycle::Lost=>"unavailable",Lifecycle::Serving if context.is_some()=>"ready",Lifecycle::Serving=>"starting"},
            "backend":self.backend,"binarySha256":self.binary_sha256,
            "contextDigest":context,"contractDigest":self.contract_digest,"pendingRequests":pending,"capabilities":capabilities})
    }
    pub async fn spawn(
        root: &Path,
        role: &str,
        packaged: bool,
        data: std::path::PathBuf,
        launch: &RoleLaunch,
    ) -> Result<Arc<Self>, String> {
        let prefix = if role == "offline_search" {
            ""
        } else {
            "protected-"
        };
        let request =
            std::fs::read(root.join(format!("packages/contracts/{prefix}request.schema.json")))
                .map_err(|e| e.to_string())?;
        let response =
            std::fs::read(root.join(format!("packages/contracts/{prefix}response.schema.json")))
                .map_err(|e| e.to_string())?;
        let mut digest = Sha256::new();
        digest.update(&request);
        digest.update(&response);
        let parse = |bytes: &[u8]| -> Result<jsonschema::Validator, String> {
            jsonschema::validator_for(
                &serde_json::from_slice::<Value>(bytes).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())
        };
        let (executable, arguments) = launch_command(role, launch, &data);
        let mut command = Command::new(&executable);
        isolate_packaged_worker_environment(&mut command, packaged);
        command.args(&arguments);
        command
            .current_dir(root)
            .env("NIOH3_STATE_ROOT", &data)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let mut child = command
            .spawn()
            .map_err(|e| format!("WORKER_START_FAILED: {e}"))?;
        let input = child.stdin.take();
        let mut output = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        // Drain bounded chunks. A full stderr pipe must never block a protected host.
        let category = format!("{role}-stderr");
        tokio::spawn(async move {
            let mut b = [0; 8192];
            let mut decoder = crate::storage::Utf8LogDecoder::default();
            while let Ok(n) = stderr.read(&mut b).await {
                if n == 0 {
                    break;
                }
                let text = decoder.push(&b[..n]);
                if !text.is_empty() {
                    crate::storage::log(&data, &category, &text);
                }
            }
            let last = decoder.finish();
            if !last.is_empty() {
                crate::storage::log(&data, &category, &last);
            }
        });
        let worker = Arc::new(Self {
            role: role.into(),
            backend: backend_identifier(launch).to_string(),
            binary_sha256: binary_digest(&executable),
            child: Mutex::new(child),
            input: Mutex::new(input),
            pending: Mutex::new(HashMap::new()),
            lifecycle: std::sync::Mutex::new(Lifecycle::Serving),
            request_schema: parse(&request)?,
            response_schema: parse(&response)?,
            contract_digest: format!("{:x}", digest.finalize()),
            identity: Mutex::new(None),
        });
        let reader = worker.clone();
        tokio::spawn(async move {
            loop {
                let result: Result<Value, String> = async {
                    let length = output.read_u32_le().await.map_err(|e| e.to_string())?;
                    if length == 0 || length > 4_194_304 {
                        return Err("INVALID_FRAME_SIZE".into());
                    }
                    let mut bytes = vec![0; length as usize];
                    output
                        .read_exact(&mut bytes)
                        .await
                        .map_err(|e| e.to_string())?;
                    let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                    if !reader.response_schema.is_valid(&value) {
                        return Err("INVALID_WORKER_RESPONSE".into());
                    }
                    Ok(value)
                }
                .await;
                match result {
                    Ok(value) => {
                        let id = value["id"].as_str().unwrap_or("");
                        let sender = { reader.pending.lock().await.remove(id) };
                        if let Some(sender) = sender {
                            let reply = if value["ok"] == true {
                                Ok(value["result"].clone())
                            } else {
                                Err(format!(
                                    "{}: {}",
                                    value["error"]["code"].as_str().unwrap_or("WORKER_ERROR"),
                                    value["error"]["message"].as_str().unwrap_or("")
                                ))
                            };
                            let _ = sender.send(reply);
                        } else {
                            reader.fail("UNEXPECTED_RESPONSE_ID").await;
                            break;
                        }
                    }
                    Err(error) => {
                        reader.fail(&error).await;
                        break;
                    }
                }
            }
        });
        Ok(worker)
    }
    async fn fail(&self, error: &str) {
        self.lose();
        for (_, sender) in self.pending.lock().await.drain() {
            let _ = sender.send(Err(error.to_string()));
        }
        self.input.lock().await.take(); // EOF lets a protected host finish and restore.
        if self.may_force_kill() {
            let _ = self.child.lock().await.kill().await;
        }
    }
    async fn request(&self, method: &str, params: Value) -> Reply {
        if self.lifecycle() != Lifecycle::Serving {
            return Err("WORKER_UNAVAILABLE: do not replay writes".into());
        }
        let id = Uuid::new_v4().to_string();
        let message = json!({"protocol":1,"id":id,"method":method,"params":params});
        if !self.request_schema.is_valid(&message) {
            return Err(format!("INVALID_REQUEST: {method}"));
        }
        let bytes = serde_json::to_vec(&message).map_err(|e| e.to_string())?;
        if bytes.len() > 4_194_304 {
            return Err("FRAME_TOO_LARGE".into());
        }
        let (sender, receiver) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            // A timed-out request keeps its id for a late reply, but no caller
            // waits on it any more, so it does not take up a slot.
            let waiting = pending
                .values()
                .filter(|sender| !sender.is_closed())
                .count();
            if waiting >= 8 {
                return Err("TOO_MANY_REQUESTS".into());
            }
            pending.insert(id.clone(), sender);
        }
        let sent = async {
            let mut guard = self.input.lock().await;
            let input = guard
                .as_mut()
                .ok_or_else(|| "WORKER_PIPE_CLOSED".to_string())?;
            input
                .write_u32_le(bytes.len() as u32)
                .await
                .map_err(|e| e.to_string())?;
            input.write_all(&bytes).await.map_err(|e| e.to_string())?;
            input.flush().await.map_err(|e| e.to_string())
        }
        .await;
        if let Err(error) = sent {
            self.fail(&error).await;
            return Err(error);
        }
        match tokio::time::timeout(
            Duration::from_secs(if method == "shutdown" { 5 } else { 30 }),
            receiver,
        )
        .await
        {
            Ok(Ok(reply)) => reply,
            _ => {
                // Keep the pending ID for a possible late reply. Never replay it.
                if self.role == "offline_search" {
                    self.fail("WORKER_TIMEOUT").await;
                }
                Err("WORKER_TIMEOUT: outcome unknown; do not replay writes".into())
            }
        }
    }
    pub async fn handshake(&self) -> Reply {
        let mut identity = self.identity.lock().await;
        if let Some(value) = identity.as_ref() {
            return Ok(value.clone());
        }
        let result = self.request("handshake", json!({})).await?;
        if result["role"] != self.role
            || result["contract_digest"] != self.contract_digest
            || (self.role != "offline_search" && result["kill_safe"] != false)
        {
            self.fail("CONTRACT_MISMATCH").await;
            return Err("CONTRACT_MISMATCH".into());
        }
        *identity = Some(result.clone());
        Ok(result)
    }
    pub async fn call(&self, method: &str, params: Value) -> Reply {
        self.handshake().await?;
        // A protected host runs one job at a time and refuses a second one
        // before starting it, so waiting out a short background job (a scroll's
        // content preview, for example) can never replay work.
        let mut attempts = 0;
        loop {
            match self.request(method, params.clone()).await {
                Err(error) if attempts < BUSY_RETRIES && error.contains(HOST_BUSY) => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_millis(BUSY_RETRY_MILLIS)).await;
                }
                result => return result,
            }
        }
    }
    pub async fn run(&self, method: &str, params: Value) -> Reply {
        let mut job = self.call(method, params).await?;
        loop {
            match job["state"].as_str() {
                Some("queued" | "running" | "cancel_requested") => {
                    tokio::time::sleep(Duration::from_millis(80)).await;
                    job = self
                        .call("job.snapshot", json!({"job_id":job["job_id"]}))
                        .await?;
                }
                Some("failed") => return Err(job["error"].to_string()),
                Some("completed" | "cancelled") => return Ok(job["result"].clone()),
                _ => return Err(format!("UNKNOWN_JOB_STATE: {}", job["state"])),
            }
        }
    }
    pub async fn close(&self) -> bool {
        if self.lifecycle() == Lifecycle::SafelyClosed {
            return true;
        }
        if self.role == "offline_search" {
            self.fail("OFFLINE_WORKER_CLOSED").await;
            return true;
        }
        if self.lifecycle() == Lifecycle::Lost {
            return false;
        }
        match self.call("shutdown", json!({})).await {
            Ok(value) if value["safe_to_shutdown"] == true => {
                *self.lifecycle.lock().expect("worker lifecycle lock") = Lifecycle::SafelyClosed;
                self.input.lock().await.take();
                true
            }
            _ => false,
        }
    }
}
