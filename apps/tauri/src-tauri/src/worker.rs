use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
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

/// The read-only search backend one launch uses.
///
/// A shipped package that carries no staged backend manifest runs the shipped
/// Python worker. A package staged with `-WorkerBackend rust` declares the Rust
/// worker in that manifest and then runs it. A development launch may name the
/// Rust development binary; nothing selects it implicitly, and a declared but
/// unusable binary is a named failure rather than a silent Python fallback that
/// would read as a passing migration test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchBackend {
    /// The shipped `nioh3_scroll_editor.search_worker`, or its packaged EXE.
    Python,
    /// The Rust read-only worker, packaged or development.
    Rust(RustSearchLaunch),
}

/// Everything the Rust read-only worker launch needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSearchLaunch {
    pub mode: RustLaunchMode,
    pub executable: std::path::PathBuf,
    pub data_root: std::path::PathBuf,
    pub contract_dir: std::path::PathBuf,
    pub accelerator: std::path::PathBuf,
}

impl RustSearchLaunch {
    /// The exact argv the binary requires for its launch mode.
    ///
    /// The Rust worker is not a drop-in replacement for the Python module: it
    /// refuses to start without exactly one launch-mode acknowledgement and it
    /// loads the product data manifest, the shipped contracts and the
    /// accelerator identity from its own arguments.
    pub fn arguments(&self) -> Vec<String> {
        let mut arguments = vec![match self.mode {
            RustLaunchMode::Development => "--dev-preview-only",
            RustLaunchMode::Packaged => "--packaged-worker",
        }
        .to_string()];
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
        .and_then(Path::parent)
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

/// The Rust launch one role resolves to from a validated staged manifest.
pub fn packaged_rust_launch(
    manifest: &StagedBackendManifest,
    role: &str,
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
    };
    Ok((executable, launch))
}

/// Resolve the read-only search backend for one launch.
///
/// A package that carries a staged Rust manifest runs its declared Rust worker.
/// Otherwise the shipped Python worker is kept, so a stray variable in a user's
/// environment cannot change the released product. In development
/// `NIOH3_RUST_SEARCH_WORKER` selects the Rust worker explicitly and the three
/// optional variables override its paths; the defaults are the working-tree
/// paths the binary needs.
pub fn search_backend(
    root: &Path,
    packaged: bool,
    env: &RustSearchEnv,
) -> Result<SearchBackend, String> {
    if packaged {
        let Some(manifest) = staged_backend_manifest(root)? else {
            return Ok(SearchBackend::Python);
        };
        let (_, launch) = packaged_rust_launch(&manifest, "offline_search")?;
        return Ok(SearchBackend::Rust(launch));
    }
    let Some(named) = env
        .executable
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(SearchBackend::Python);
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
    Ok(SearchBackend::Rust(RustSearchLaunch {
        mode: RustLaunchMode::Development,
        executable,
        data_root: path(&env.data_root, "nioh3_scroll_editor/data"),
        contract_dir: path(&env.contract_dir, "packages/contracts"),
        accelerator: path(&env.accelerator, "bin/nioh3_seed_accelerator.dll"),
    }))
}

/// The protected backend one launch uses.
///
/// Production always keeps the shipped protected worker. A development launch
/// may explicitly name the Rust protected binary instead; nothing selects it
/// implicitly, and a missing file is a named failure rather than a silent
/// Python fallback that would read as a passing migration signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtectedBackend {
    /// The shipped `nioh3_scroll_editor.protected_worker`, or its packaged EXE.
    Python,
    /// `crates/nioh3-protected`'s development binary.
    Rust(RustProtectedLaunch),
}

/// Everything the development Rust protected launch needs that is not already
/// supplied per role (the state root is the host's own data directory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustProtectedLaunch {
    pub mode: RustLaunchMode,
    pub executable: std::path::PathBuf,
    pub data_root: std::path::PathBuf,
    pub contract_dir: std::path::PathBuf,
    pub accelerator: std::path::PathBuf,
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
) -> Result<ProtectedBackend, String> {
    if packaged {
        let Some(manifest) = staged_backend_manifest(root)? else {
            return Ok(ProtectedBackend::Python);
        };
        let (_, search) = packaged_rust_launch(&manifest, "save")?;
        // Both protected roles must resolve to the same declared binary and the
        // same roots, or the package is not internally consistent.
        let (_, runtime) = packaged_rust_launch(&manifest, "runtime")?;
        if runtime.executable != search.executable {
            return Err("WORKER_BACKEND_ROLE_BINARY: protected roles disagree".into());
        }
        return Ok(ProtectedBackend::Rust(RustProtectedLaunch {
            mode: search.mode,
            executable: search.executable,
            data_root: search.data_root,
            contract_dir: search.contract_dir,
            accelerator: search.accelerator,
        }));
    }
    let Some(named) = env
        .executable
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(ProtectedBackend::Python);
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
    Ok(ProtectedBackend::Rust(RustProtectedLaunch {
        mode: RustLaunchMode::Development,
        executable,
        data_root: path(&env.data_root, "nioh3_scroll_editor/data"),
        contract_dir: path(&env.contract_dir, "packages/contracts"),
        accelerator: path(&env.accelerator, "bin/nioh3_seed_accelerator.dll"),
    }))
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
    dead: AtomicBool,
    safely_closed: AtomicBool,
    request_schema: jsonschema::Validator,
    response_schema: jsonschema::Validator,
    contract_digest: String,
    identity: Mutex<Option<Value>>,
}

/// The executable plus argv one role launches with.
///
/// A Rust search launch replaces the `offline_search` command line; the protected
/// roles take the Rust protected command line only when that backend was
/// selected, and otherwise keep the shipped worker and its `--role` argument.
/// The root arguments come from the resolved launch, never from the working
/// directory, so a packaged host cannot pass the packaged EXE a working-tree
/// path.
pub(crate) fn launch_command(
    root: &Path,
    role: &str,
    packaged: bool,
    backend: &SearchBackend,
    protected: &ProtectedBackend,
    state_root: &Path,
) -> (std::path::PathBuf, Vec<String>) {
    if let ProtectedBackend::Rust(launch) = protected {
        if role != "offline_search" {
            return (
                launch.executable.clone(),
                launch.arguments(role, state_root),
            );
        }
    }
    if let SearchBackend::Rust(launch) = backend {
        return (launch.executable.clone(), launch.arguments());
    }
    let executable = if packaged {
        root.join(if role == "offline_search" {
            "worker/nioh3-search-worker.exe"
        } else {
            "worker/nioh3-protected-worker.exe"
        })
    } else {
        std::path::PathBuf::from(std::env::var("NIOH3_PYTHON").unwrap_or_else(|_| "python".into()))
    };
    let mut arguments = Vec::new();
    if !packaged {
        arguments.push("-u".to_string());
        arguments.push("-m".to_string());
        arguments.push(
            if role == "offline_search" {
                "nioh3_scroll_editor.search_worker"
            } else {
                "nioh3_scroll_editor.protected_worker"
            }
            .to_string(),
        );
    }
    if role != "offline_search" {
        arguments.push("--role".to_string());
        arguments.push(role.to_string());
    }
    (executable, arguments)
}

/// Which backend graph this launch resolved to, for diagnostics and support.
pub fn backend_identifier(
    packaged: bool,
    backend: &SearchBackend,
    protected: &ProtectedBackend,
) -> String {
    let rust = |mode: RustLaunchMode| match mode {
        RustLaunchMode::Packaged => "rust-packaged",
        RustLaunchMode::Development => "rust-development",
    };
    match (backend, protected) {
        (SearchBackend::Rust(launch), _) => rust(launch.mode).to_string(),
        (_, ProtectedBackend::Rust(launch)) => rust(launch.mode).to_string(),
        _ if packaged => "python-packaged".to_string(),
        _ => "python-development".to_string(),
    }
}

/// Sha256 of the spawned executable, or `None` when it cannot be read.
///
/// A published worker EXE hashes fine; the development Python fallback is a bare
/// `python` on `PATH`, which may not be a regular file, so absence is reported
/// rather than judged.
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
    let search = search_backend(root, packaged, &rust_search_env())?;
    let protected = protected_backend(root, packaged, &rust_protected_env())?;
    // A packaged launch must run the binary the manifest declared, so the
    // declared sha256 and the staged bytes are compared before anything spawns.
    if packaged {
        if let Some(manifest) = staged_backend_manifest(root)? {
            let (executable, _) =
                launch_command(root, role, packaged, &search, &protected, state_root);
            verify_declared_binary(&manifest, role, &executable)?;
        }
    }
    Ok(launch_command(
        root, role, packaged, &search, &protected, state_root,
    ))
}

impl Worker {
    /// A broken pipe alone is not permission to replace a protected owner.
    pub async fn can_replace(&self) -> bool {
        if self.safely_closed.load(Ordering::SeqCst) {
            return true;
        }
        self.dead.load(Ordering::SeqCst)
            && self.child.lock().await.try_wait().ok().flatten().is_some()
    }
    #[cfg(test)]
    pub async fn disconnect_for_test(&self) {
        self.fail("TEST_TRANSPORT_LOST").await;
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
        json!({"role":self.role,"connection":if self.safely_closed.load(Ordering::SeqCst){"closed"}
            else if self.dead.load(Ordering::SeqCst){"unavailable"}else if context.is_some(){"ready"}else{"starting"},
            "backend":self.backend,"binarySha256":self.binary_sha256,
            "contextDigest":context,"contractDigest":self.contract_digest,"pendingRequests":pending})
    }
    pub async fn spawn(
        root: &Path,
        role: &str,
        packaged: bool,
        data: std::path::PathBuf,
        backend: &SearchBackend,
        protected: &ProtectedBackend,
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
        let (executable, arguments) =
            launch_command(root, role, packaged, backend, protected, &data);
        let mut command = Command::new(&executable);
        command.args(&arguments);
        command
            .current_dir(root)
            .env("NIOH3_STATE_ROOT", &data)
            .env("PYTHONUTF8", "1")
            .env("PYTHONIOENCODING", "utf-8")
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
            backend: backend_identifier(packaged, backend, protected),
            binary_sha256: binary_digest(&executable),
            child: Mutex::new(child),
            input: Mutex::new(input),
            pending: Mutex::new(HashMap::new()),
            dead: AtomicBool::new(false),
            safely_closed: AtomicBool::new(false),
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
        self.dead.store(true, Ordering::SeqCst);
        for (_, sender) in self.pending.lock().await.drain() {
            let _ = sender.send(Err(error.to_string()));
        }
        self.input.lock().await.take(); // EOF lets a protected host finish and restore.
        if self.role == "offline_search" {
            let _ = self.child.lock().await.kill().await;
        }
    }
    async fn request(&self, method: &str, params: Value) -> Reply {
        if self.dead.load(Ordering::SeqCst) {
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
            if pending.len() >= 8 {
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
        self.request(method, params).await
    }
    pub async fn run(&self, method: &str, params: Value) -> Reply {
        let mut job = self.call(method, params).await?;
        while job["state"] == "running" || job["state"] == "cancel_requested" {
            tokio::time::sleep(Duration::from_millis(80)).await;
            job = self
                .call("job.snapshot", json!({"job_id":job["job_id"]}))
                .await?;
        }
        if job["state"] == "failed" {
            return Err(job["error"].to_string());
        }
        Ok(job["result"].clone())
    }
    pub async fn close(&self) -> bool {
        if self.safely_closed.load(Ordering::SeqCst) {
            return true;
        }
        if self.role == "offline_search" {
            self.fail("OFFLINE_WORKER_CLOSED").await;
            return true;
        }
        if self.dead.load(Ordering::SeqCst) {
            return false;
        }
        match self.call("shutdown", json!({})).await {
            Ok(value) if value["safe_to_shutdown"] == true => {
                self.safely_closed.store(true, Ordering::SeqCst);
                self.input.lock().await.take();
                true
            }
            _ => false,
        }
    }
}
