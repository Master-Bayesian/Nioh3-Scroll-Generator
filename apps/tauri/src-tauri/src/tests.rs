use crate::broker::Broker;
use crate::worker::{
    protected_backend, search_backend, ProtectedBackend, RustProtectedEnv, RustSearchEnv,
    SearchBackend,
};
use base64::Engine;
use serde_json::json;

#[test]
fn qq_invite_accepts_only_the_official_group_protocol_shape() {
    let html = r#"<script>var qsig = "tencent:\/\/groupwpa\/?subcmd=all\u0026param=7b2267726f757055696e223a313130363330323437397d";</script>"#;
    assert_eq!(
        crate::qq_protocol_from_html(html).as_deref(),
        Some("tencent://groupwpa/?subcmd=all&param=7b2267726f757055696e223a313130363330323437397d&jump_from=webapi")
    );
    assert!(crate::qq_protocol_from_html(
        r#"var qsig = "https:\/\/attacker.invalid\/?subcmd=all\u0026param=7b22";"#
    )
    .is_none());
    assert!(crate::qq_protocol_from_html(
        r#"var qsig = "tencent:\/\/groupwpa\/?subcmd=other\u0026param=7b2267726f757055696e223a313130363330323437397d";"#
    )
    .is_none());
    assert!(crate::qq_protocol_from_html(
        r#"var qsig = "tencent:\/\/groupwpa\/?subcmd=all\u0026param=7b2267726f757055696e223a393939393939393939397d";"#
    )
    .is_none());
}

#[test]
fn javascript_signature_and_package_paths() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../test-fixtures/signed-update.json")).unwrap();
    let key: [u8; 32] = base64::engine::general_purpose::STANDARD
        .decode(fixture["publicKey"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let manifest: crate::update::Manifest =
        serde_json::from_value(fixture["manifest"].clone()).unwrap();
    crate::update::validate_with_key(&manifest, key).unwrap();
    assert!(
        crate::update::validate(&manifest).is_err(),
        "Synthetic key cannot sign production updates"
    );
    let mut changed = manifest.clone();
    changed.notes.push('!');
    assert!(crate::update::validate_with_key(&changed, key).is_err());
    for path in [
        "../escape",
        "C:/escape",
        "NUL.zip",
        "COM1.zip",
        "con .zip",
        "a/../b",
        "a\\b",
        "/absolute",
        "trailing.",
    ] {
        assert!(!crate::package::safe_relative(path), "{path}");
    }
}

#[test]
fn real_update_helper_preserves_rollback_until_acknowledgement() {
    use crate::package::{hash_file, ENTRY};
    use std::path::Path;
    fn fixture(root: &Path, version: &str, valid: bool) {
        let paths = [
            ENTRY,
            "worker/nioh3-search-worker.exe",
            "worker/nioh3-protected-worker.exe",
            "packages/contracts/request.schema.json",
            "packages/contracts/response.schema.json",
            "packages/contracts/protected-request.schema.json",
            "packages/contracts/protected-response.schema.json",
        ];
        let mut files = vec![];
        for name in paths {
            let path = root.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            if name == ENTRY && valid {
                std::fs::copy(
                    std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
                        .join("System32/whoami.exe"),
                    &path,
                )
                .unwrap();
            } else {
                std::fs::write(&path, format!("{name} {version}")).unwrap();
            }
            files.push(json!({"path":name,"size":std::fs::metadata(&path).unwrap().len(),"sha256":hash_file(&path).unwrap()}));
        }
        crate::storage::write_json(
            &root.join("build-manifest.json"),
            &json!({"schema":"nioh3-tauri-manifest/v1","version":version,"files":files}),
        )
        .unwrap();
    }
    let root = std::env::temp_dir().join(format!("nioh3-tauri-install-{}", uuid::Uuid::new_v4()));
    let profile = root.join("profile");
    let cache = profile.join("updates");
    let folder = cache.join(uuid::Uuid::new_v4().to_string());
    let stage = folder.join("package");
    let target = root.join("installed");
    fixture(&target, "0.7.1", true);
    std::fs::write(target.join("uninstall.exe"), "installer-owned-uninstaller").unwrap();
    fixture(&stage, "0.7.2", true);
    let helper = cache.join("apply-update.ps1");
    std::fs::write(&helper, include_str!("../apply-update.ps1")).unwrap();
    let apply = |source: &Path| {
        let mut cmd = std::process::Command::new("powershell.exe");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&helper)
        .args(["-ProcessId", "2147483647", "-Target"])
        .arg(&target)
        .arg("-Staged")
        .arg(source)
        .arg("-ManifestHash")
        .arg(hash_file(&source.join("build-manifest.json")).unwrap())
        .arg("-Profile")
        .arg(&profile);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        cmd.output().unwrap()
    };
    let result = apply(&stage);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report = cache.join("last-update-result.json");
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
    let previous = std::path::PathBuf::from(receipt["previous"].as_str().unwrap());
    assert!(previous.exists());
    assert_eq!(
        std::fs::read(target.join("uninstall.exe")).unwrap(),
        b"installer-owned-uninstaller"
    );
    let before = std::fs::read(target.join("build-manifest.json")).unwrap();
    let broken = root.join("broken");
    fixture(&broken, "0.7.3", false);
    assert!(!apply(&broken).status.success());
    assert_eq!(
        std::fs::read(target.join("build-manifest.json")).unwrap(),
        before
    );
    assert!(std::fs::read_dir(&root).unwrap().all(|e| {
        let n = e.unwrap().file_name().to_string_lossy().into_owned();
        !n.contains(".failed-") && !n.contains(".update-")
    }));
    crate::storage::write_json(&report, &receipt).unwrap();
    let updater = crate::update::Updater::new(cache.clone());
    std::fs::write(previous.join("user-save.bin"), "User file").unwrap();
    assert!(updater.cleanup(&target).unwrap_err().contains("USER_FILES"));
    assert!(previous.join("user-save.bin").exists());
    std::fs::remove_file(previous.join("user-save.bin")).unwrap();
    updater.cleanup(&target.canonicalize().unwrap()).unwrap();
    assert!(!previous.exists());
    assert!(!folder.exists());
    assert_eq!(
        std::fs::read(target.join("build-manifest.json")).unwrap(),
        before
    );
    updater.cleanup(&target).unwrap();
}

#[tokio::test]
async fn real_worker_search_validation_and_private_transfers() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap();
    let data = std::env::temp_dir().join(format!("nioh3-tauri-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&data).unwrap();
    let _identity = bind_development_worker_identity();
    let broker = Broker::new(root, data, false);
    let hello = broker
        .dispatch("core:handshake", json!(null))
        .await
        .unwrap();
    assert_eq!(hello["role"], "offline_search");
    let params = json!({"context_digest":hello["context"]["context_digest"],"result_count":2,"page_trials":100000,"job_trials":1000000,"allow_cpu_fallback":true,"resume_token":null,"query":{"playthrough":3,"rarity":4,"level":180,"primary_effect_ids":[44634],"required_secondary_ids":[],"required_secondary_id_groups":[],"grace_effect_id":null,"minimum_roll_percent_by_effect_id":[],"auxiliary":{"required_terrain_effect_keys":[],"required_terrain_effect_key_groups":[],"required_special_rule_keys":[],"required_special_rule_key_groups":[],"required_enemy_lookup_keys":[],"required_enemy_lookup_key_groups":[]}}});
    let mut invalid = params.clone();
    invalid["result_count"] = json!(true);
    assert!(broker
        .dispatch("core:start", invalid)
        .await
        .unwrap_err()
        .contains("INVALID_REQUEST"));
    let started = broker.dispatch("core:start", params.clone()).await.unwrap();
    let mut job = started.clone();
    for _ in 0..300 {
        job = broker
            .dispatch("core:snapshot", started["job_id"].clone())
            .await
            .unwrap();
        if ["completed", "failed", "cancelled"].contains(&job["state"].as_str().unwrap()) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    }
    assert_eq!(job["state"], "completed", "{job}");
    assert_eq!(job["candidates"].as_array().unwrap().len(), 2);
    assert_eq!(job["candidates"][0]["effects"][0]["effect_id"], 44634);
    let retained = broker
        .dispatch(
            "review:retain",
            json!({"job_id":job["job_id"],"candidate_id":job["candidates"][0]["candidate_id"]}),
        )
        .await
        .unwrap();
    assert_eq!(retained.as_object().unwrap().len(), 1);
    assert!(retained["reference_id"].is_string());
    assert_eq!(
        broker.dispatch("core:current", json!(null)).await.unwrap()["submitted"],
        params
    );
    for method in [
        "save.template",
        "save.count_edit_source",
        "save.materialize_live_many",
        "runtime.export",
    ] {
        assert_eq!(
            broker
                .dispatch("operations:execute", json!({"method":method,"params":{}}))
                .await
                .unwrap_err(),
            "PRIVATE_OR_UNKNOWN_OPERATION"
        );
    }
    let host = broker.host("save").await.unwrap();
    host.handshake().await.unwrap();
    assert!(host.close().await);
    assert!(host.close().await, "Closing a safe host twice must succeed");
    assert!(broker.shutdown().await);
}

#[test]
fn support_log_keeps_complete_chunked_diagnostics_with_iso_timestamps() {
    let data = std::env::temp_dir().join(format!("nioh3-log-test-{}", uuid::Uuid::new_v4()));
    let message = format!(
        "save_path=C:/Users/player/save record_hex={}",
        "ab".repeat(9000)
    );
    crate::storage::log(&data, "worker-request", &message);
    let text = std::fs::read_to_string(data.join("logs/desktop.log")).unwrap();
    assert!(text.contains("T"));
    assert!(text.contains("Z [worker-request part=1/3]"));
    assert!(text.contains("[worker-request part=3/3]"));
    assert!(text.contains("save_path=C:/Users/player/save"));
    assert_eq!(text.matches("ab").count(), 9000);
    std::fs::remove_dir_all(data).unwrap();
}

#[test]
fn support_log_tail_crosses_rotated_file_boundaries_in_order() {
    let data = std::env::temp_dir().join(format!("nioh3-log-tail-test-{}", uuid::Uuid::new_v4()));
    let logs = data.join("logs");
    std::fs::create_dir_all(&logs).unwrap();
    std::fs::write(logs.join("desktop.2.log"), "oldest\n").unwrap();
    std::fs::write(logs.join("desktop.1.log"), "previous\n").unwrap();
    std::fs::write(logs.join("desktop.log"), "current\n").unwrap();
    assert_eq!(
        crate::storage::support_log_tail(&data, 1024),
        "oldest\nprevious\ncurrent\n"
    );
    assert_eq!(
        crate::storage::support_log_tail(&data, 17),
        "previous\ncurrent\n"
    );
    std::fs::write(logs.join("desktop.log"), "错误详情\n").unwrap();
    assert_eq!(crate::storage::support_log_tail(&data, 7), "详情\n");
    std::fs::remove_dir_all(data).unwrap();
}

#[test]
fn support_tail_skips_every_partial_utf8_prefix() {
    let data = std::env::temp_dir().join(format!("nioh3-utf8-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(data.join("logs")).unwrap();
    std::fs::write(data.join("logs/desktop.log"), "错😀OK\n").unwrap();
    for count in 4..=6 {
        assert_eq!(crate::storage::support_log_tail(&data, count), "OK\n");
    }
    std::fs::remove_dir_all(data).unwrap();
}

#[test]
fn single_oversized_payload_cannot_exceed_the_segment_limit() {
    let data = std::env::temp_dir().join(format!("nioh3-cap-{}", uuid::Uuid::new_v4()));
    crate::storage::log(&data, "worker-request", &"字".repeat(8_000_000));
    let files: Vec<_> = std::fs::read_dir(data.join("logs"))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(files.len() <= 5);
    assert!(files
        .iter()
        .all(|file| file.metadata().unwrap().len() <= 4 * 1024 * 1024));
    std::fs::remove_dir_all(data).unwrap();
}

#[test]
fn first_actionable_error_is_pinned_across_rotation_and_cleanup_errors() {
    let data = std::env::temp_dir().join(format!("nioh3-first-{}", uuid::Uuid::new_v4()));
    crate::storage::log(&data, "worker-error", "operation=first native=idle-gate");
    crate::storage::log(&data, "worker-request", &"x".repeat(5_000_000));
    crate::storage::log(&data, "worker-error", "cleanup-error");
    let pinned = crate::storage::first_failure(&data).unwrap();
    assert!(pinned.contains("operation=first native=idle-gate"));
    assert!(!pinned.contains("cleanup-error"));
    std::fs::remove_dir_all(data).unwrap();
}

#[test]
fn worker_stderr_preserves_utf8_across_arbitrary_pipe_splits() {
    let value = "失败😀 C:/存档/SAVEDATA.BIN";
    for split in 0..=value.len() {
        let mut decoder = crate::storage::Utf8LogDecoder::default();
        let mut decoded = decoder.push(&value.as_bytes()[..split]);
        decoded.push_str(&decoder.push(&value.as_bytes()[split..]));
        decoded.push_str(&decoder.finish());
        assert_eq!(decoded, value);
    }
    let mut decoder = crate::storage::Utf8LogDecoder::default();
    assert_eq!(decoder.push(&[0xff, 0xe4]), "\u{FFFD}");
    assert_eq!(decoder.finish(), "\u{FFFD}");
}

#[tokio::test]
async fn dead_protected_worker_is_replaced_only_after_its_process_exits() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let data = std::env::temp_dir().join(format!("nioh3-recover-{}", uuid::Uuid::new_v4()));
    let broker = Broker::new(root, data.clone(), false);
    let old = broker.host("save").await.unwrap();
    old.handshake().await.unwrap();
    assert!(!old.can_replace().await);
    old.disconnect_for_test().await; // EOF, not process kill; no game dispatch.
    for _ in 0..100 {
        if old.can_replace().await {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(old.can_replace().await);
    let replacement = broker.host("save").await.unwrap();
    assert!(!std::sync::Arc::ptr_eq(&old, &replacement));
    replacement.handshake().await.unwrap();
    assert!(broker.shutdown().await);
    let _ = std::fs::remove_dir_all(data);
}

/// A protected host owns live game or save state, so a broken pipe may only
/// close its input and let it finish: the process is never force-killed, and a
/// transport failure is never reported as a proven safe close.
#[tokio::test]
async fn a_broken_pipe_never_force_kills_a_protected_host() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let data = std::env::temp_dir().join(format!("nioh3-guard-{}", uuid::Uuid::new_v4()));
    let broker = Broker::new(root, data.clone(), false);
    let host = broker.host("save").await.unwrap();
    host.handshake().await.unwrap();
    host.disconnect_for_test().await; // EOF, exactly the Tauri broken-pipe rule.
    assert!(
        !host.close().await,
        "a dead protected transport must never report a proven safe close"
    );
    for _ in 0..200 {
        if host.can_replace().await {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        host.can_replace().await,
        "the protected host must exit on EOF"
    );
    let status = host
        .exit_status()
        .await
        .expect("the protected host must have exited");
    assert_eq!(
        status.code(),
        Some(0),
        "a protected host must finish by itself, never by force"
    );
    let _ = std::fs::remove_dir_all(data);
}

/// The read-only search worker owns no game or save state, so it stays the one
/// role a transport failure may reap.
#[tokio::test]
async fn a_broken_pipe_reclaims_the_readonly_search_worker() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let data = std::env::temp_dir().join(format!("nioh3-reap-{}", uuid::Uuid::new_v4()));
    let _identity = bind_development_worker_identity();
    let broker = Broker::new(root, data.clone(), false);
    let host = broker.host("offline_search").await.unwrap();
    host.handshake().await.unwrap();
    host.disconnect_for_test().await;
    for _ in 0..200 {
        if host.can_replace().await {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(host.can_replace().await);
    let status = host.exit_status().await.expect("the search worker exited");
    assert!(
        !status.success(),
        "the read-only worker is the role this layer may terminate"
    );
    let _ = std::fs::remove_dir_all(data);
}

/// The development Rust worker selection never changes the shipped product and
/// never silently falls back to Python once an operator has asked for it.
#[test]
fn a_packaged_host_without_a_staged_manifest_keeps_the_shipped_search_worker() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let named = RustSearchEnv {
        executable: Some(root.join("Cargo.toml").display().to_string()),
        data_root: Some("elsewhere".to_string()),
        ..RustSearchEnv::default()
    };
    assert_eq!(
        search_backend(root, true, &named).unwrap(),
        SearchBackend::Python,
        "a packaged host must ignore the development selection"
    );
    assert_eq!(
        search_backend(root, false, &RustSearchEnv::default()).unwrap(),
        SearchBackend::Python,
        "an unset selection keeps the shipped worker"
    );
    assert_eq!(
        search_backend(
            root,
            false,
            &RustSearchEnv {
                executable: Some("   ".to_string()),
                ..RustSearchEnv::default()
            }
        )
        .unwrap(),
        SearchBackend::Python,
        "a blank selection keeps the shipped worker"
    );
    let missing = search_backend(
        root,
        false,
        &RustSearchEnv {
            executable: Some(root.join("does-not-exist.exe").display().to_string()),
            ..RustSearchEnv::default()
        },
    )
    .expect_err("a named but absent binary fails closed");
    assert!(missing.starts_with("RUST_WORKER_MISSING"), "{missing}");
}

/// The development launch must run the real Rust binary with its own arguments,
/// never the Python module, and must use the working-tree paths by default.
#[test]
fn the_development_rust_launch_names_the_binary_and_its_paths() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let binary = root.join("Cargo.toml");
    let backend = search_backend(
        root,
        false,
        &RustSearchEnv {
            executable: Some(binary.display().to_string()),
            ..RustSearchEnv::default()
        },
    )
    .unwrap();
    let SearchBackend::Rust(launch) = backend else {
        panic!("an existing binary must select the Rust backend");
    };
    assert_eq!(launch.executable, binary);
    assert_eq!(launch.data_root, root.join("nioh3_scroll_editor/data"));
    assert_eq!(launch.contract_dir, root.join("packages/contracts"));
    assert_eq!(
        launch.accelerator,
        root.join("bin/nioh3_seed_accelerator.dll")
    );
    let arguments = launch.arguments();
    assert_eq!(arguments[0], "--dev-preview-only");
    assert_eq!(
        arguments,
        vec![
            "--dev-preview-only",
            "--data-root",
            &launch.data_root.display().to_string(),
            "--contract-dir",
            &launch.contract_dir.display().to_string(),
            "--accelerator",
            &launch.accelerator.display().to_string(),
        ]
    );
    assert!(
        !arguments
            .iter()
            .any(|value| value.contains("search_worker")),
        "the development launch must not run the Python module"
    );

    // Explicit overrides win, so an isolated profile or harness can point the
    // worker at its own data root without touching the working tree.
    let overridden = search_backend(
        root,
        false,
        &RustSearchEnv {
            executable: Some(binary.display().to_string()),
            data_root: Some("/tmp/data".to_string()),
            contract_dir: Some("/tmp/contracts".to_string()),
            accelerator: Some("/tmp/accelerator.dll".to_string()),
            ..RustSearchEnv::default()
        },
    )
    .unwrap();
    let SearchBackend::Rust(overridden) = overridden else {
        panic!("overrides keep the Rust backend");
    };
    assert_eq!(overridden.data_root, std::path::PathBuf::from("/tmp/data"));
    assert_eq!(
        overridden.contract_dir,
        std::path::PathBuf::from("/tmp/contracts")
    );
    assert_eq!(
        overridden.accelerator,
        std::path::PathBuf::from("/tmp/accelerator.dll")
    );
}

/// The development protected selection never changes the shipped product and
/// never silently falls back to Python once an operator has asked for it.
#[test]
fn a_packaged_host_without_a_staged_manifest_keeps_the_shipped_protected_worker() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let named = RustProtectedEnv {
        executable: Some(root.join("Cargo.toml").display().to_string()),
        data_root: Some("elsewhere".to_string()),
        ..RustProtectedEnv::default()
    };
    assert_eq!(
        protected_backend(root, true, &named).unwrap(),
        ProtectedBackend::Python,
        "a packaged host must ignore the development selection"
    );
    assert_eq!(
        protected_backend(root, false, &RustProtectedEnv::default()).unwrap(),
        ProtectedBackend::Python,
        "an unset selection keeps the shipped worker"
    );
    assert_eq!(
        protected_backend(
            root,
            false,
            &RustProtectedEnv {
                executable: Some("   ".to_string()),
                ..RustProtectedEnv::default()
            }
        )
        .unwrap(),
        ProtectedBackend::Python,
        "a blank selection keeps the shipped worker"
    );
    let missing = protected_backend(
        root,
        false,
        &RustProtectedEnv {
            executable: Some(root.join("does-not-exist.exe").display().to_string()),
            ..RustProtectedEnv::default()
        },
    )
    .expect_err("a named but absent binary fails closed");
    assert!(
        missing.starts_with("RUST_PROTECTED_WORKER_MISSING"),
        "{missing}"
    );
}

/// The development protected launch must run the real Rust binary with its own
/// arguments, never the Python module, and must carry the role plus state root.
#[test]
fn the_development_protected_launch_names_the_binary_and_its_paths() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let binary = root.join("Cargo.toml");
    let backend = protected_backend(
        root,
        false,
        &RustProtectedEnv {
            executable: Some(binary.display().to_string()),
            ..RustProtectedEnv::default()
        },
    )
    .unwrap();
    let ProtectedBackend::Rust(launch) = backend else {
        panic!("an existing binary must select the Rust protected backend");
    };
    assert_eq!(launch.executable, binary);
    assert_eq!(launch.data_root, root.join("nioh3_scroll_editor/data"));
    assert_eq!(launch.contract_dir, root.join("packages/contracts"));
    assert_eq!(
        launch.accelerator,
        root.join("bin/nioh3_seed_accelerator.dll")
    );
    let state = std::path::PathBuf::from(r"D:\state");
    let arguments = launch.arguments("save", &state);
    assert_eq!(
        arguments,
        vec![
            "--role",
            "save",
            "--dev-protected-only",
            "--state-root",
            &state.display().to_string(),
            "--data-root",
            &launch.data_root.display().to_string(),
            "--contract-dir",
            &launch.contract_dir.display().to_string(),
            "--accelerator",
            &launch.accelerator.display().to_string(),
        ]
    );
    assert!(
        !arguments
            .iter()
            .any(|value| value.contains("protected_worker")),
        "the development launch must not run the Python module"
    );
}

/// Build a minimal but structurally real staged Rust package: the manifest the
/// staging tool writes, both declared worker EXEs at their packaged names, and
/// the package-confined data/contract/helper roots the workers resolve.
/// The lock every test that binds, forces, or observes the packaged session
/// version holds.
///
/// The packaged session value is process-wide by design - one session, one
/// identity - so a test that installs one must not overlap another test that
/// reads the same value. Tests that need a resolved session take this guard for
/// their whole body, which makes a parallel run deterministic without changing
/// the production shape under test.
fn session_version_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The game build the real development-graph tests bind their workers to.
///
/// The development Python search graph refuses to start without an identity, and
/// a parallel test must not write process-wide state, so a test that starts that
/// graph names this build explicitly on its own thread. It is a game build, never
/// a product version.
const DEVELOPMENT_GAME_FILE_VERSION: &str = "2.0.2.0";

/// Bind the explicit development identity a real-worker test starts under.
///
/// The guard restores whatever was bound before, so the binding never outlives
/// the test that asked for it.
fn bind_development_worker_identity() -> crate::worker::DevelopmentGameFileVersionBinding {
    crate::worker::bind_development_game_file_version_for_test(DEVELOPMENT_GAME_FILE_VERSION)
}

/// Install a resolved packaged session version for one test.
///
/// The packaged resolvers in this module always establish the session's
/// installed version before they hand an argv to a worker; the real source reads
/// the game executable this machine happens to have. A test that only wants the
/// resolver's argv shape therefore installs a fixed version through the same
/// test hook the packaged resolver reads, so the argv under test is the argv a
/// host with a verified install would spawn.
///
/// The pinned value is the Tauri package version, which is the only four-part
/// spelling already recorded in this crate. It keeps every test that binds a
/// session version in agreement without teaching them a product version.
fn bind_packaged_session_version(version: &str) {
    crate::worker::set_packaged_game_file_version_for_test(Some(version.to_string()))
        .expect("the test hook installs the session version");
    crate::worker::set_packaged_game_file_version_error_for_test(None)
        .expect("the test hook clears the forced failure");
}

/// Release the packaged session version a test installed, so no later test can
/// inherit it. Every test that binds one calls this before it returns.
fn release_packaged_session_version() {
    crate::worker::set_packaged_game_file_version_for_test(None)
        .expect("the test hook releases the session version");
    crate::worker::set_packaged_game_file_version_error_for_test(None)
        .expect("the test hook releases the forced failure");
}

fn staged_rust_package(name: &str) -> std::path::PathBuf {
    use std::path::Path;
    let root = std::env::temp_dir().join(format!("nioh3-staged-{name}-{}", uuid::Uuid::new_v4()));
    let write = |relative: &str, body: &str| {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    };
    write("worker/nioh3-search-worker.exe", "staged search worker");
    write(
        "worker/nioh3-protected-worker.exe",
        "staged protected worker",
    );
    write(
        "worker/runtime/nioh3_scroll_editor/data/tables.bin",
        "tables",
    );
    write(
        "worker/runtime/bin/nioh3_seed_accelerator.dll",
        "shipped helper",
    );
    write("packages/contracts/request.schema.json", "{}");
    write("packages/contracts/response.schema.json", "{}");
    write("packages/contracts/protected-request.schema.json", "{}");
    write("packages/contracts/protected-response.schema.json", "{}");
    let sha = |relative: &str| crate::package::hash_file(&root.join(relative)).unwrap();
    // Build the manifest with the real staged paths already inside the JSON
    // strings: substituting a Windows path into serialized JSON afterwards would
    // need its backslashes re-escaped.
    let data_root = root
        .join("worker/runtime/nioh3_scroll_editor/data")
        .display()
        .to_string();
    let contract_dir = root.join("packages/contracts").display().to_string();
    let manifest = json!({
        "schema": "nioh3-worker-backend/v1",
        "backend": "rust",
        "binaries": [
            {"packagedName": "nioh3-search-worker.exe", "sha256": sha("worker/nioh3-search-worker.exe")},
            {"packagedName": "nioh3-protected-worker.exe", "sha256": sha("worker/nioh3-protected-worker.exe")},
        ],
        "invocation": {
            "offline_search": {"mode": "packaged", "binary": "nioh3-search-worker.exe", "argv": [
                "--packaged-worker",
                "--data-root", data_root.clone(),
                "--contract-dir", contract_dir.clone()]},
            "save": {"mode": "packaged", "binary": "nioh3-protected-worker.exe", "argv": [
                "--role", "save",
                "--state-root", "<state root>",
                "--data-root", data_root.clone(),
                "--contract-dir", contract_dir.clone()]},
            "runtime": {"mode": "packaged", "binary": "nioh3-protected-worker.exe", "argv": [
                "--role", "runtime",
                "--state-root", "<state root>",
                "--data-root", data_root.clone(),
                "--contract-dir", contract_dir.clone()]},
        },
        "launchContract": {
            "schema": "nioh3-worker-launch-contract/v1",
            "roles": ["offline_search", "save", "runtime"],
            "stateRoot": {"policy": "broker-injected-external", "placeholder": "<state root>", "packageConfined": false},
        },
    });
    std::fs::write(
        root.join("worker/worker-backend.json"),
        manifest.to_string(),
    )
    .unwrap();
    assert!(Path::new(&root.join("worker/nioh3-search-worker.exe")).is_file());
    root
}

/// The packaged resolver is driven from the staged manifest, so every role
/// receives the roots and the binary the package actually staged.
#[test]
fn packaged_manifest_resolves_every_role_to_the_staged_binary_and_roots() {
    use crate::worker::{normalize_path, resolve_role_launch, staged_backend_manifest};
    let _guard = session_version_guard();
    let root = staged_rust_package("packaged-resolve");
    let staged = staged_backend_manifest(&root)
        .unwrap()
        .expect("staged manifest");
    let staged_data = root
        .join("worker")
        .join("runtime")
        .join("nioh3_scroll_editor")
        .join("data");
    let staged_contracts = root.join("packages").join("contracts");
    let staged_helper = root
        .join("worker")
        .join("runtime")
        .join("bin")
        .join("nioh3_seed_accelerator.dll");
    bind_packaged_session_version("0.7.5.0");
    assert_eq!(normalize_path(&staged.data_root), staged_data);
    assert_eq!(normalize_path(&staged.contract_dir), staged_contracts);
    assert_eq!(normalize_path(&staged.accelerator), staged_helper);
    let state = std::env::temp_dir().join("nioh3-state-outside-package");
    for role in ["offline_search", "save", "runtime"] {
        let (executable, arguments) = resolve_role_launch(&root, role, true, &state).unwrap();
        assert_eq!(executable.parent().unwrap(), root.join("worker"));
        assert_eq!(
            normalize_path(&executable),
            root.join("worker").join(if role == "offline_search" {
                "nioh3-search-worker.exe"
            } else {
                "nioh3-protected-worker.exe"
            })
        );
        let value = |flag: &str| {
            let index = arguments
                .iter()
                .position(|token| token == flag)
                .unwrap_or_else(|| panic!("{role} argv lacks {flag}: {arguments:?}"));
            arguments[index + 1].clone()
        };
        assert_eq!(
            normalize_path(std::path::Path::new(&value("--data-root"))),
            staged_data
        );
        assert_eq!(
            normalize_path(std::path::Path::new(&value("--contract-dir"))),
            staged_contracts
        );
        assert_eq!(
            normalize_path(std::path::Path::new(&value("--accelerator"))),
            staged_helper
        );
        // F3: the packaged and development shapes pin the helper identically.
        assert!(
            arguments.contains(&"--accelerator".to_string()),
            "{role} must pin the accelerator explicitly: {arguments:?}"
        );
        if role == "offline_search" {
            assert_eq!(arguments[0], "--packaged-worker");
            assert!(!arguments.iter().any(|token| token == "--dev-preview-only"));
        } else {
            assert_eq!(arguments[0], "--role");
            assert_eq!(arguments[1], role);
            assert_eq!(value("--state-root"), state.display().to_string());
            assert!(!arguments
                .iter()
                .any(|token| token == "--dev-protected-only"));
        }
        // The staged resolution carries the session's resolved identity too.
        assert_eq!(value("--game-file-version"), "0.7.5.0");
    }
    release_packaged_session_version();
    let _ = std::fs::remove_dir_all(&root);
}

/// A staged package that declares a binary or a root it does not carry is a
/// named refusal, never a silent fallback to the shipped Python worker.
#[test]
fn packaged_manifest_fails_closed_for_missing_or_escaping_declarations() {
    use crate::worker::{search_backend, staged_backend_manifest, RustSearchEnv};
    let _guard = session_version_guard();
    let root = staged_rust_package("packaged-refuse");
    bind_packaged_session_version("0.7.5.0");
    assert!(search_backend(&root, true, &RustSearchEnv::default()).is_ok());

    let missing = staged_rust_package("packaged-missing");
    std::fs::remove_file(missing.join("worker/nioh3-protected-worker.exe")).unwrap();
    let error = search_backend(&missing, true, &RustSearchEnv::default())
        .expect_err("a declared but absent binary must refuse");
    assert!(
        error.starts_with("RUST_WORKER_MISSING"),
        "{error} (a fallback to Python would hide this)"
    );

    let escaping = staged_rust_package("packaged-escaping");
    let manifest_path = escaping.join("worker/worker-backend.json");
    let outside = std::env::temp_dir().join("nioh3-outside-data-root");
    std::fs::create_dir_all(&outside).unwrap();
    // Substitute through the JSON value so a Windows path is escaped for us.
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    for role in ["offline_search", "save", "runtime"] {
        let argv = manifest["invocation"][role]["argv"].as_array_mut().unwrap();
        let index = argv
            .iter()
            .position(|token| token == "--data-root")
            .expect("declared data root");
        argv[index + 1] = json!(outside.display().to_string());
    }
    std::fs::write(&manifest_path, manifest.to_string()).unwrap();
    let error = search_backend(&escaping, true, &RustSearchEnv::default())
        .expect_err("a path escaping the package must refuse");
    assert!(
        error.starts_with("WORKER_BACKEND_ESCAPES_PACKAGE"),
        "{error}"
    );

    let unsupported = staged_rust_package("packaged-unsupported");
    let manifest_path = unsupported.join("worker/worker-backend.json");
    let text = std::fs::read_to_string(&manifest_path).unwrap();
    std::fs::write(&manifest_path, text.replace("\"rust\"", "\"python\"")).unwrap();
    let error =
        staged_backend_manifest(&unsupported).expect_err("an unknown declared backend must refuse");
    assert!(error.contains("WORKER_BACKEND_UNSUPPORTED"), "{error}");

    // A package without a manifest keeps the shipped worker exactly as before.
    let plain = std::env::temp_dir().join(format!("nioh3-unstaged-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(plain.join("worker")).unwrap();
    assert_eq!(
        search_backend(&plain, true, &RustSearchEnv::default()).unwrap(),
        SearchBackend::Python
    );
    for path in [&missing, &escaping, &unsupported, &plain] {
        let _ = std::fs::remove_dir_all(path);
    }
    release_packaged_session_version();
    let _ = std::fs::remove_dir_all(&outside);
}

/// A staged binary whose bytes no longer match the declared sha256 is refused by
/// name. The manifest is a declaration, not an oracle, so replacing the file
/// after staging must not be launchable.
#[test]
fn packaged_manifest_refuses_a_binary_that_changed_after_staging() {
    use crate::worker::{resolve_role_launch, staged_backend_manifest, verify_declared_binary};
    let _guard = session_version_guard();
    let root = staged_rust_package("packaged-mutation");
    bind_packaged_session_version("0.7.5.0");
    let manifest = staged_backend_manifest(&root)
        .unwrap()
        .expect("staged manifest");
    let binary = root.join("worker").join("nioh3-search-worker.exe");
    // The untouched file passes the declared identity.
    verify_declared_binary(&manifest, "offline_search", &binary).expect("declared bytes match");

    let mut bytes = std::fs::read(&binary).unwrap();
    bytes.extend_from_slice(b"tampered-after-staging");
    std::fs::write(&binary, bytes).unwrap();
    let error = verify_declared_binary(&manifest, "offline_search", &binary)
        .expect_err("a changed binary must be refused");
    assert!(error.starts_with("RUST_WORKER_CHANGED"), "{error}");

    // The packaged resolver propagates the same refusal, so a tampered package
    // cannot reach a launch through the acceptance entry point either.
    let error = resolve_role_launch(
        &root,
        "offline_search",
        true,
        &std::env::temp_dir().join("nioh3-mutation-state"),
    )
    .expect_err("the resolver must refuse a changed binary");
    assert!(error.starts_with("RUST_WORKER_CHANGED"), "{error}");
    release_packaged_session_version();
    let _ = std::fs::remove_dir_all(&root);
}

/// The staged manifest declares `<runtime>/a/b` with POSIX separators while the
/// packaged host's root is canonical (`\\?\...`). A canonical path is never
/// normalized, so a mixed-separator path is looked up literally: without the
/// separator conversion every declared resource reads as missing and no packaged
/// launch can start its workers. This is the shape `resource_dir()` hands the
/// host, not the raw environment path the development acceptance uses.
#[test]
fn packaged_manifest_resolves_placeholder_paths_under_a_canonical_root() {
    use crate::worker::{normalize_path, resolve_role_launch, staged_backend_manifest};
    let _guard = session_version_guard();
    let root = std::env::temp_dir().join(format!("nioh3-canonical-{}", uuid::Uuid::new_v4()));
    let write = |relative: &str, body: &str| {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    };
    write("worker/nioh3-search-worker.exe", "staged search worker");
    write(
        "worker/nioh3-protected-worker.exe",
        "staged protected worker",
    );
    write(
        "worker/runtime/nioh3_scroll_editor/data/tables.bin",
        "tables",
    );
    write("worker/runtime/bin/nioh3_seed_accelerator.dll", "helper");
    write("packages/contracts/request.schema.json", "{}");
    write("packages/contracts/response.schema.json", "{}");
    write("packages/contracts/protected-request.schema.json", "{}");
    write("packages/contracts/protected-response.schema.json", "{}");
    let sha = |relative: &str| crate::package::hash_file(&root.join(relative)).unwrap();
    // Exactly what `tools/stage_rust_workers.py` writes: a `<runtime>` placeholder
    // with forward slashes, never a pre-resolved absolute path.
    let manifest = json!({
        "schema": "nioh3-worker-backend/v1",
        "backend": "rust",
        "binaries": [
            {"packagedName": "nioh3-search-worker.exe", "sha256": sha("worker/nioh3-search-worker.exe")},
            {"packagedName": "nioh3-protected-worker.exe", "sha256": sha("worker/nioh3-protected-worker.exe")},
        ],
        "invocation": {
            "offline_search": {"mode": "packaged", "binary": "nioh3-search-worker.exe", "argv": [
                "--packaged-worker",
                "--data-root", "<runtime>/worker/runtime/nioh3_scroll_editor/data",
                "--contract-dir", "<runtime>/packages/contracts"]},
            "save": {"mode": "packaged", "binary": "nioh3-protected-worker.exe", "argv": [
                "--role", "save",
                "--state-root", "<state root>",
                "--data-root", "<runtime>/worker/runtime/nioh3_scroll_editor/data",
                "--contract-dir", "<runtime>/packages/contracts"]},
            "runtime": {"mode": "packaged", "binary": "nioh3-protected-worker.exe", "argv": [
                "--role", "runtime",
                "--state-root", "<state root>",
                "--data-root", "<runtime>/worker/runtime/nioh3_scroll_editor/data",
                "--contract-dir", "<runtime>/packages/contracts"]},
        },
        "launchContract": {
            "schema": "nioh3-worker-launch-contract/v1",
            "roles": ["offline_search", "save", "runtime"],
            "stateRoot": {"policy": "broker-injected-external", "placeholder": "<state root>", "packageConfined": false},
        },
    });
    std::fs::write(
        root.join("worker/worker-backend.json"),
        manifest.to_string(),
    )
    .unwrap();

    // `resource_dir()` hands the host the canonical form, which is why the raw
    // development override never reproduced this.
    let canonical = std::path::PathBuf::from(format!(r"\\?\{}", root.display()));
    assert!(
        canonical.to_string_lossy().starts_with(r"\\?\"),
        "the packaged root must be canonical: {}",
        canonical.display()
    );
    let staged = staged_backend_manifest(&canonical)
        .unwrap()
        .expect("staged manifest");
    assert_eq!(
        normalize_path(&staged.data_root),
        normalize_path(&root.join("worker/runtime/nioh3_scroll_editor/data"))
    );
    assert!(
        staged.data_root.is_dir(),
        "the declared data root must resolve: {}",
        staged.data_root.display()
    );
    assert!(staged.contract_dir.is_dir());
    assert!(staged.accelerator.is_file());
    let state = std::env::temp_dir().join("nioh3-canonical-state");
    bind_packaged_session_version("0.7.5.0");
    for role in ["offline_search", "save", "runtime"] {
        let (executable, arguments) = resolve_role_launch(&canonical, role, true, &state)
            .unwrap_or_else(|error| panic!("{role} did not resolve: {error}"));
        assert!(
            executable.is_file(),
            "{role} binary {}",
            executable.display()
        );
        let value = |flag: &str| {
            let index = arguments
                .iter()
                .position(|token| token == flag)
                .unwrap_or_else(|| panic!("{role} argv lacks {flag}: {arguments:?}"));
            arguments[index + 1].clone()
        };
        assert!(
            std::path::Path::new(&value("--data-root")).is_dir(),
            "{role} data root {}",
            value("--data-root")
        );
        assert!(std::path::Path::new(&value("--contract-dir")).is_dir());
        assert!(std::path::Path::new(&value("--accelerator")).is_file());
    }
    release_packaged_session_version();
    let _ = std::fs::remove_dir_all(&root);
}

/// The installed-version grammar is exactly the four-component spelling the
/// worker binaries parse, so a value this host produces is never refused
/// downstream for its shape.
#[test]
fn the_game_version_grammar_accepts_only_four_numeric_components() {
    use crate::game_version::GameFileVersion;
    assert_eq!(
        GameFileVersion::parse("2.0.2.0").unwrap().dotted(),
        "2.0.2.0"
    );
    assert_eq!(
        GameFileVersion::parse("65535.0.65535.1").unwrap().dotted(),
        "65535.0.65535.1"
    );
    assert_eq!(
        GameFileVersion::parse("0.0.0.0").unwrap().to_string(),
        "0.0.0.0",
        "an all-zero build is still a four-component spelling"
    );

    for rejected in [
        "2.02",           // two components
        "2.0.2",          // three components
        "2.0.2.0.1",      // five components
        "2.0.2.0.0.0",    // six components
        "",               // empty
        "2.0..0",         // empty component
        "2.0.2.x",        // non-numeric
        "2.0.2.-1",       // signed
        "2.0.2.65536",    // above u16
        "2.0.2.99999999", // far above u16
        "v2.0.2.0",       // prefixed
        "2.0.2.0 ",       // trailing space
        " 2.0.2.0",       // leading space
        "  2.0.1.0  ",    // surrounding space
        "2.0. 2.0",       // inner space
        "2.0.2.0\t",      // trailing tab
        "\t2.0.2.0",      // leading tab
        "2.0.2.0\n",      // trailing newline
        "\n2.0.2.0",      // leading newline
    ] {
        let error = GameFileVersion::parse(rejected).unwrap_err();
        assert!(
            error.starts_with("GAME_VERSION_MALFORMED"),
            "{rejected:?} was accepted or mislabelled: {error}"
        );
    }
}

/// The host grammar sits inside the worker grammar, with no trimming on either
/// side.
///
/// Both parsers split on `.` and require four components; the workers then parse
/// each component with `u16::from_str`
/// (`crates/nioh3-worker/src/main.rs:182-198`,
/// `crates/nioh3-protected/src/main.rs:187-202`), which reads no whitespace. The
/// host is stricter in exactly one direction: it never produces the leading `+`
/// a component may carry, so refusing that spelling keeps the host value one both
/// workers accept.
#[test]
fn the_host_grammar_stays_inside_the_worker_grammar() {
    use crate::game_version::GameFileVersion;

    // The worker's own shape, spelled out here so the direction of the relation
    // is executed rather than asserted in prose.
    fn worker_accepts(raw: &str) -> bool {
        let parts: Vec<&str> = raw.split('.').collect();
        parts.len() == 4 && parts.iter().all(|part| part.parse::<u16>().is_ok())
    }

    for accepted in ["2.0.2.0", "0.0.0.0", "65535.65535.65535.65535", "2.0.1.0"] {
        assert!(GameFileVersion::parse(accepted).is_ok(), "{accepted:?}");
        assert!(worker_accepts(accepted), "{accepted:?}");
    }
    for raw in [
        "2.0.2.0 ",
        " 2.0.2.0",
        "  2.0.1.0  ",
        "2.0.2.0\t",
        "2.0.2.0\n",
        "\n2.0.2.0",
        "2.0. 2.0",
    ] {
        assert!(
            GameFileVersion::parse(raw).is_err(),
            "the host must not trim: {raw:?}"
        );
        assert!(
            !worker_accepts(raw),
            "the worker parser does not trim either: {raw:?}"
        );
    }
    // The single place the two accepted sets differ, in the safe direction.
    assert!(worker_accepts("+2.0.1.0"));
    assert!(GameFileVersion::parse("+2.0.1.0").is_err());
}

/// One session reads the installed executable once and every later question is
/// answered from that same value, so all roles share one identity.
#[test]
fn the_session_version_is_detected_once_and_then_cached() {
    use crate::game_version::{
        FileVersionReader, GameFileVersion, GameFileVersionSource, GameVersionSource,
    };
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Counting {
        reads: AtomicUsize,
        answer: Result<GameFileVersion, String>,
    }
    impl FileVersionReader for Counting {
        fn read(&self, _executable: &Path) -> Result<GameFileVersion, String> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            self.answer.clone()
        }
    }

    // A real file is required because the source refuses a non-file path before
    // it ever asks the reader.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let executable = root.join("Cargo.toml");
    let reader = Counting {
        reads: AtomicUsize::new(0),
        answer: Ok(GameFileVersion::parse("2.0.2.0").unwrap()),
    };
    let source = GameFileVersionSource::new(
        reader,
        GameVersionSource::with_development_executable(Some(executable)),
    );
    for _ in 0..5 {
        assert_eq!(source.resolve().unwrap().dotted(), "2.0.2.0");
    }
    assert_eq!(
        source.reads_for_test(),
        1,
        "the executable must be read exactly once per session"
    );

    // A failure is cached the same way: a session that cannot establish the
    // identity must not retry into a different answer.
    let failing_reader = Counting {
        reads: AtomicUsize::new(0),
        answer: Err("GAME_VERSION_UNREADABLE: no version resource".to_string()),
    };
    let failing = GameFileVersionSource::new(
        failing_reader,
        GameVersionSource::with_development_executable(Some(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
        )),
    );
    for _ in 0..3 {
        let error = failing.resolve().unwrap_err();
        assert_eq!(error.code, "GAME_VERSION_UNREADABLE");
    }
    assert_eq!(
        failing.reads_for_test(),
        1,
        "a failed resolution is cached rather than retried"
    );
}

/// Every production role receives the identical resolved version, and the
/// packaged resolver never falls back to a constant or to the legacy identity.
#[test]
fn every_packaged_role_receives_the_same_resolved_game_file_version() {
    use crate::worker::{
        launch_command, protected_backend, resolve_role_launch, search_backend, ProtectedBackend,
        RustSearchEnv, SearchBackend,
    };
    let _guard = session_version_guard();
    bind_packaged_session_version("0.7.5.0");
    let root = staged_rust_package("version-plumbing");
    let state = std::env::temp_dir().join("nioh3-version-plumbing-state");

    let search = search_backend(&root, true, &RustSearchEnv::default()).unwrap();
    let protected =
        protected_backend(&root, true, &crate::worker::RustProtectedEnv::default()).unwrap();
    let SearchBackend::Rust(search_launch) = &search else {
        panic!("a staged package must select the Rust search backend");
    };
    assert_eq!(
        search_launch.game_file_version.as_deref(),
        Some("0.7.5.0"),
        "the packaged search launch must carry the resolved version"
    );
    assert!(
        !search_launch.legacy_test_context,
        "packaged production must never select the legacy identity"
    );
    let ProtectedBackend::Rust(protected_launch) = &protected else {
        panic!("a staged package must select the Rust protected backend");
    };
    assert_eq!(
        protected_launch.game_file_version.as_deref(),
        Some("0.7.5.0")
    );
    assert!(!protected_launch.legacy_test_context);

    // The value the argv carries must be byte-identical for all three roles.
    let mut seen: Vec<String> = Vec::new();
    for role in ["offline_search", "save", "runtime"] {
        let (_, arguments) = launch_command(&root, role, true, &search, &protected, &state);
        let index = arguments
            .iter()
            .position(|token| token == "--game-file-version")
            .unwrap_or_else(|| panic!("{role} argv lacks the version flag: {arguments:?}"));
        seen.push(arguments[index + 1].clone());
        assert!(
            !arguments
                .iter()
                .any(|token| token == "--legacy-test-context"),
            "{role} must not carry the legacy opt-in: {arguments:?}"
        );
    }
    assert_eq!(
        seen,
        vec![
            "0.7.5.0".to_string(),
            "0.7.5.0".to_string(),
            "0.7.5.0".to_string()
        ],
        "every production role must receive the same version"
    );

    // `resolve_role_launch` is the packaged gate's entry point; it must agree.
    let (_, arguments) = resolve_role_launch(&root, "runtime", true, &state).unwrap();
    let index = arguments
        .iter()
        .position(|token| token == "--game-file-version")
        .expect("the gate entry point must carry the version");
    assert_eq!(arguments[index + 1], "0.7.5.0");

    release_packaged_session_version();
    let _ = std::fs::remove_dir_all(&root);
}

/// A host that cannot establish the installed identity refuses before any
/// worker starts, rather than passing a default or an environment guess.
#[test]
fn a_packaged_role_without_a_resolved_version_fails_closed() {
    use crate::worker::{packaged_rust_launch, staged_backend_manifest};
    let root = staged_rust_package("version-fail-closed");
    let manifest = staged_backend_manifest(&root)
        .unwrap()
        .expect("staged manifest");
    // The packaged constructor has no version-less shape: it takes the resolved
    // value, so a session that cannot resolve one cannot build a launch at all.
    let launch = packaged_rust_launch(&manifest, "offline_search", "2.0.0.2")
        .unwrap()
        .1;
    assert_eq!(launch.game_file_version.as_deref(), Some("2.0.0.2"));
    assert!(
        !launch.legacy_test_context,
        "the packaged shape must never carry the legacy opt-in"
    );
    let arguments = launch.arguments();
    assert!(arguments.contains(&"--game-file-version".to_string()));
    assert!(arguments.contains(&"2.0.0.2".to_string()));
    // There is no fallback constant anywhere in the produced argv.
    assert!(
        !arguments
            .iter()
            .any(|token| token == "--legacy-test-context"),
        "{arguments:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// The packaged gate refuses before it can hand any worker an argv that names no
/// version. The refusal is the resolver's structured failure, not a placeholder
/// version and not the legacy opt-in.
#[test]
fn the_packaged_gate_refuses_when_the_session_version_is_unavailable() {
    use crate::worker::{
        resolve_role_launch, set_packaged_game_file_version_error_for_test,
        set_packaged_game_file_version_for_test,
    };
    let _guard = session_version_guard();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let state = std::env::temp_dir().join("nioh3-unavailable-version-state");
    set_packaged_game_file_version_for_test(None).unwrap();
    // A staged manifest would let the resolver's packaged branch answer first;
    // the shape under test is a package whose manifest is not there, so the
    // gate is the only thing that can establish the session version.
    set_packaged_game_file_version_error_for_test(Some(
        "GAME_EXECUTABLE_NOT_FOUND: no installed Nioh3.exe under the known Steam roots".to_string(),
    ))
    .unwrap();
    for role in ["offline_search", "save", "runtime"] {
        let error = resolve_role_launch(root, role, true, &state)
            .expect_err("a packaged role without a resolved version must be refused");
        assert!(
            error.starts_with("GAME_EXECUTABLE_NOT_FOUND"),
            "{role} was refused with the wrong code: {error}"
        );
    }
    set_packaged_game_file_version_error_for_test(None).unwrap();
}

/// One session resolves the installed version once, and every role - Rust
/// search, Rust protected, and the shipped Python search worker - receives that
/// same value, so no two workers can bind to different identities.
#[test]
fn the_session_resolves_one_version_and_every_role_carries_it() {
    use crate::game_version::{
        FileVersionReader, GameFileVersion, GameFileVersionSource, GameVersionSource,
    };
    use crate::worker::{launch_command, resolve_role_launch, ProtectedBackend, SearchBackend};
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let _guard = session_version_guard();

    struct Counting {
        reads: AtomicUsize,
    }
    impl FileVersionReader for Counting {
        fn read(&self, _executable: &Path) -> Result<GameFileVersion, String> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            GameFileVersion::parse("0.7.5.0")
        }
    }

    // A real file is required because the source refuses a non-file path before
    // it ever asks the reader.
    let source = GameFileVersionSource::new(
        Counting {
            reads: AtomicUsize::new(0),
        },
        GameVersionSource::with_development_executable(Some(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
        )),
    );
    let resolved = source.resolve().expect("the fake reader answers").dotted();

    // The packaged resolvers repeat the one session value this host established.
    bind_packaged_session_version(&resolved);
    let root = staged_rust_package("session-one-version");
    let state = std::env::temp_dir().join("nioh3-session-one-version-state");
    let search = search_backend(&root, true, &RustSearchEnv::default()).unwrap();
    let protected =
        protected_backend(&root, true, &crate::worker::RustProtectedEnv::default()).unwrap();

    // The packaged argv for every role names that one value.
    let mut seen: Vec<(String, String)> = Vec::new();
    for role in ["offline_search", "save", "runtime"] {
        let (_, arguments) = launch_command(&root, role, true, &search, &protected, &state);
        let index = arguments
            .iter()
            .position(|token| token == "--game-file-version")
            .unwrap_or_else(|| panic!("{role} argv lacks the version flag: {arguments:?}"));
        seen.push((role.to_string(), arguments[index + 1].clone()));
    }
    // The shipped Python search worker is the fallback graph: it runs the same
    // role through the same argv builder and must answer identically.
    let (_, python_arguments) = launch_command(
        &root,
        "offline_search",
        true,
        &SearchBackend::Python,
        &ProtectedBackend::Python,
        &state,
    );
    let python_index = python_arguments
        .iter()
        .position(|token| token == "--game-file-version")
        .expect("the Python search argv must name the session version");
    seen.push((
        "python-offline_search".to_string(),
        python_arguments[python_index + 1].clone(),
    ));
    for (role, version) in &seen {
        assert_eq!(
            version, &resolved,
            "{role} must receive the session's one resolved version"
        );
    }
    assert!(
        resolve_role_launch(&root, "offline_search", true, &state)
            .expect("the packaged gate resolves the Python graph")
            .1
            .iter()
            .any(|token| token == "--game-file-version"),
        "the packaged gate's argv names the session version for the Python graph"
    );
    assert_eq!(
        source.reads_for_test(),
        1,
        "the session must read the executable exactly once"
    );
    release_packaged_session_version();
    let _ = std::fs::remove_dir_all(&root);
}

/// If a packaged role ever reached the argv builder without a resolved version,
/// the argv it produced would carry an explicit non-version token. No worker
/// parser accepts that token, so the worker refuses to start instead of
/// accepting jobs under an undefined identity - and the legacy opt-in is still
/// never selected.
#[test]
fn an_unresolved_version_never_becomes_a_version_or_the_legacy_opt_in() {
    use crate::worker::{
        push_python_identity_for_test, set_packaged_game_file_version_error_for_test,
    };
    let _guard = session_version_guard();
    set_packaged_game_file_version_error_for_test(Some(
        "GAME_VERSION_UNREADABLE: no version resource on D:\\Games\\Nioh3\\Nioh3.exe".to_string(),
    ))
    .unwrap();
    let mut arguments: Vec<String> = Vec::new();
    push_python_identity_for_test(&mut arguments, true, None, false);
    assert_eq!(
        arguments.first().map(String::as_str),
        Some("--game-file-version"),
        "the flag is named even when the value is unavailable: {arguments:?}"
    );
    let token = arguments.get(1).cloned().unwrap_or_default();
    assert!(
        token.starts_with("GAME_VERSION_UNAVAILABLE:"),
        "the unavailable identity must be typed: {token}"
    );
    assert!(
        !token.contains("GAME_EXECUTABLE_UNREADABLE"),
        "the token must keep the resolver's own code, not the generic one: {token}"
    );
    assert!(
        crate::game_version::GameFileVersion::parse(&token).is_err(),
        "no worker may read the unavailable token as a version"
    );
    assert!(
        !arguments
            .iter()
            .any(|argument| argument == "--legacy-test-context"),
        "packaged production must never select the legacy identity: {arguments:?}"
    );
    set_packaged_game_file_version_error_for_test(None).unwrap();
}

/// Discovery reads only Steam roots that can be named without walking a
/// filesystem, and the version resource parse is exact.
#[test]
fn discovery_is_bounded_to_named_steam_roots() {
    use crate::game_version::GameFileVersionSource;
    use crate::game_version::GameVersionSource;
    use crate::game_version::WindowsFileVersionReader;

    // A version source for a directory, not a file, refuses by name.
    let source = GameFileVersionSource::new(
        WindowsFileVersionReader,
        GameVersionSource::with_development_executable(Some(std::path::PathBuf::from(env!(
            "CARGO_MANIFEST_DIR"
        )))),
    );
    let error = source.resolve().unwrap_err();
    assert_eq!(
        error.code,
        "GAME_EXECUTABLE_UNREADABLE",
        "{}",
        error.message()
    );

    // The named-executable override does not silently become a disk search: an
    // absent file is a refusal, never a fallback to discovery.
    let missing = GameFileVersionSource::new(
        WindowsFileVersionReader,
        GameVersionSource::with_development_executable(Some(std::path::PathBuf::from(
            r"D:\definitely-absent\Nioh3.exe",
        ))),
    );
    let error = missing.resolve().unwrap_err();
    assert_eq!(
        error.code,
        "GAME_EXECUTABLE_UNREADABLE",
        "{}",
        error.message()
    );
}

/// The `libraryfolders.vdf` reader takes only Valve's `path` pairs.
#[test]
fn libraryfolders_reader_takes_only_declared_paths() {
    let vdf = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
		"apps"
		{
			"1325200"		"123"
		}
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}
"#;
    let declared = crate::game_version::declared_libraries_for_test(vdf);
    assert_eq!(
        declared,
        vec![
            std::path::PathBuf::from(r"C:\Program Files (x86)\Steam"),
            std::path::PathBuf::from(r"D:\SteamLibrary"),
        ],
        "only path pairs are library roots, and each appears once"
    );
    assert!(
        !declared
            .iter()
            .any(|path| path.to_string_lossy().contains("1325200")),
        "an app id is not a library root"
    );
}

/// Discovery derives one exact expected path per known Steam root and library,
/// takes only the candidates that exist, and deduplicates by the directory a
/// candidate actually resolves to - so one install never reads as an ambiguous
/// pair and two installs never read as one.
#[test]
fn discovery_derives_exact_candidate_paths_from_temp_fixtures() {
    use crate::game_version::derive_candidate_paths_for_test;
    use std::path::PathBuf;

    let libraries: Vec<PathBuf> = vec![
        PathBuf::from(r"C:\Program Files (x86)\Steam"),
        PathBuf::from(r"D:\SteamLibrary"),
        // A library named twice in different spellings is still one library.
        PathBuf::from(r"d:\steamlibrary"),
    ];
    let candidates = derive_candidate_paths_for_test(&libraries);
    assert_eq!(
        candidates,
        vec![
            PathBuf::from(r"C:\Program Files (x86)\Steam")
                .join("steamapps")
                .join("common")
                .join("Nioh3")
                .join("Nioh3.exe"),
            PathBuf::from(r"D:\SteamLibrary")
                .join("steamapps")
                .join("common")
                .join("Nioh3")
                .join("Nioh3.exe"),
        ],
        "one exact expected path per distinct library, in declaration order"
    );
    for candidate in &candidates {
        assert_eq!(
            candidate
                .file_name()
                .map(|name| name.to_string_lossy().to_string()),
            Some("Nioh3.exe".to_string()),
            "every candidate is the exact game executable, never a directory scan"
        );
        assert!(
            candidate
                .to_string_lossy()
                .replace('/', "\\")
                .ends_with(r"steamapps\common\Nioh3\Nioh3.exe"),
            "every candidate is the Steam install layout: {}",
            candidate.display()
        );
    }
}

/// Zero, one, and more than one trustworthy candidate are three different
/// answers, and only exactly one is an identity this host will bind to.
#[test]
fn discovery_answers_zero_one_and_many_distinctly() {
    use crate::game_version::{trustworthy_game_executable_for_test, GameFileVersionSource};
    use std::path::PathBuf;

    let empty: Vec<PathBuf> = Vec::new();
    let error =
        trustworthy_game_executable_for_test(&empty).expect_err("no candidate is not an identity");
    assert!(
        error.starts_with("GAME_EXECUTABLE_NOT_FOUND"),
        "zero candidates must be named as not-found: {error}"
    );

    let root = std::env::temp_dir().join(format!("nioh3-candidates-{}", uuid::Uuid::new_v4()));
    let make = |relative: &str| {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not a real executable").unwrap();
        path
    };
    let first = make("one/Nioh3.exe");
    let one = trustworthy_game_executable_for_test(std::slice::from_ref(&first))
        .expect("exactly one candidate is the identity");
    assert_eq!(
        one.canonicalize().unwrap(),
        first.canonicalize().unwrap(),
        "a single candidate is canonicalized before it is returned"
    );

    // The same file named twice is still one install, not an ambiguous pair.
    let aliased = trustworthy_game_executable_for_test(&[first.clone(), first.clone()])
        .expect("one install named twice is not ambiguous");
    assert_eq!(aliased.canonicalize().unwrap(), one.canonicalize().unwrap());

    let second = make("two/Nioh3.exe");
    let error = trustworthy_game_executable_for_test(&[first.clone(), second.clone()])
        .expect_err("two distinct installs are ambiguous");
    assert!(
        error.starts_with("GAME_EXECUTABLE_AMBIGUOUS"),
        "two candidates must be named as ambiguous: {error}"
    );

    // A named candidate that is present but is not a readable file is refused by
    // name, and the refusal never resolves to a different candidate. A directory
    // is the shape of a wrong file name, so it must not be reported as "not
    // found" - that text would send a player to reinstall a game they have.
    let directory = root.join("three");
    std::fs::create_dir_all(&directory).unwrap();
    let error = trustworthy_game_executable_for_test(&[directory])
        .expect_err("a directory is not an executable");
    assert!(
        error.starts_with("GAME_EXECUTABLE_UNREADABLE"),
        "an unusable candidate must be named: {error}"
    );

    // A candidate set where nothing was ever present is still "not found".
    let absent = root.join("four").join("Nioh3.exe");
    let error = trustworthy_game_executable_for_test(&[absent])
        .expect_err("an absent candidate is not an identity");
    assert!(
        error.starts_with("GAME_EXECUTABLE_NOT_FOUND"),
        "absence must stay distinct from an unusable install: {error}"
    );

    // The reader is never consulted for a candidate set this host refused, and
    // the source refuses a directory before it ever asks the reader.
    let source = GameFileVersionSource::new(
        crate::game_version::WindowsFileVersionReader,
        crate::game_version::GameVersionSource::with_development_executable(Some(
            root.join("three"),
        )),
    );
    let error = source.resolve().unwrap_err();
    assert_eq!(
        error.code,
        "GAME_EXECUTABLE_UNREADABLE",
        "{}",
        error.message()
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Every refusal a player reads has to be actionable in this host, so none of
/// them names an environment override: a packaged host reads no executable
/// variable, and a dead-end instruction is worse than no instruction.
#[test]
fn the_discovery_refusals_name_no_environment_override() {
    use crate::game_version::trustworthy_game_executable_for_test;

    let root = std::env::temp_dir().join(format!("nioh3-refusals-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let make = |relative: &str| {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not a real executable").unwrap();
        path
    };

    let not_found = trustworthy_game_executable_for_test(&[root.join("absent/Nioh3.exe")])
        .expect_err("an absent candidate is not an identity");
    let unreadable = trustworthy_game_executable_for_test(std::slice::from_ref(&root))
        .expect_err("a directory is not an executable");
    let ambiguous =
        trustworthy_game_executable_for_test(&[make("one/Nioh3.exe"), make("two/Nioh3.exe")])
            .expect_err("two distinct installs are ambiguous");

    for message in [&not_found, &unreadable, &ambiguous] {
        assert!(
            !message.contains("NIOH3_"),
            "a refusal must not name a variable this host does not read: {message}"
        );
        assert!(
            !message.contains("environment"),
            "a refusal must not point at an override this host cannot apply: {message}"
        );
    }
    // The two refusals a player can act on name the bounded Steam lookup and the
    // Steam-side repair, so the text describes what this host really does.
    for message in [&not_found, &unreadable] {
        assert!(message.contains("Steam"), "{message}");
    }
    assert!(
        unreadable.contains(&root.display().to_string()),
        "the unreadable refusal names the path it refused: {unreadable}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// A Windows system executable whose version resource every supported install
/// ships, chosen under the system directory instead of a hardcoded drive path.
#[cfg(windows)]
fn system_executable_with_a_version_resource() -> std::path::PathBuf {
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let system32 = std::path::Path::new(&system_root).join("System32");
    for name in [
        "whoami.exe",
        "cmd.exe",
        "reg.exe",
        "icacls.exe",
        "tasklist.exe",
    ] {
        let candidate = system32.join(name);
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!("no system executable under {}", system32.display());
}

/// The shipped reader executes the real Windows version-resource API, and the
/// source a packaged host caches answers every later question from that one read.
///
/// The executable comes from the system directory, so this test needs no game
/// install and copies nothing into the repository, and it asserts the structural
/// shape - four `u16` components that survive the argv grammar - rather than any
/// particular OS version.
#[cfg(windows)]
#[test]
fn the_shipped_reader_reads_a_real_system_executable_once_per_session() {
    use crate::game_version::{
        FileVersionReader, GameFileVersion, GameFileVersionSource, GameVersionSource,
        WindowsFileVersionReader,
    };

    let executable = system_executable_with_a_version_resource();
    let direct = WindowsFileVersionReader
        .read(&executable)
        .unwrap_or_else(|error| {
            panic!(
                "the shipped reader must read {}: {error}",
                executable.display()
            )
        });
    let dotted = direct.dotted();
    let components: Vec<&str> = dotted.split('.').collect();
    assert_eq!(components.len(), 4, "{dotted}");
    for component in &components {
        assert!(
            component.parse::<u16>().is_ok(),
            "{component:?} in {dotted} is not a 16-bit component"
        );
    }
    assert_eq!(
        GameFileVersion::parse(&dotted).expect("the argv grammar reads what the reader produced"),
        direct,
        "the reader's value and the worker-facing spelling agree"
    );

    // The session source a packaged host caches answers from its first read, so
    // one launch binds every role to one identity without touching the disk
    // again.
    let source = GameFileVersionSource::new(
        WindowsFileVersionReader,
        GameVersionSource::with_development_executable(Some(executable)),
    );
    let resolved = source.resolve().expect("a real system executable resolves");
    assert_eq!(source.reads_for_test(), 1, "the first resolve reads once");
    assert_eq!(
        source
            .resolve()
            .expect("a cached answer is still an answer"),
        resolved
    );
    assert_eq!(
        source.reads_for_test(),
        1,
        "every later resolve answers from the session cache"
    );
    assert_eq!(
        resolved.dotted(),
        dotted,
        "the cached value is the read value"
    );
}

/// Off Windows there is no version-resource API to run, so the shipped reader
/// has to refuse by name; this keeps that branch covered rather than skipped.
#[cfg(not(windows))]
#[test]
fn the_shipped_reader_refuses_a_windows_version_resource_off_windows() {
    use crate::game_version::{FileVersionReader, WindowsFileVersionReader};

    let error = WindowsFileVersionReader
        .read(std::path::Path::new(r"C:\Windows\System32\whoami.exe"))
        .expect_err("a Windows version resource cannot be read off Windows");
    assert!(
        error.starts_with("GAME_VERSION_UNSUPPORTED_PLATFORM"),
        "{error}"
    );
}

/// Acceptance probe for the Python packaged gate.
///
/// It is `#[ignore]`d so it never runs in a normal build or test pass; the gate
/// invokes it with `--ignored --exact --nocapture` and reads the single JSON line
/// this prints. That keeps the argv the gate compares against the argv the real
/// host resolver produces, rather than a second copy of the staging tool's rules.
#[test]
#[ignore]
fn dump_role_launch_for_acceptance() {
    use crate::worker::resolve_role_launch;
    let root = std::path::PathBuf::from(std::env::var("NIOH3_ACCEPTANCE_ROOT").unwrap());
    let role = std::env::var("NIOH3_ACCEPTANCE_ROLE").unwrap();
    let packaged = std::env::var("NIOH3_ACCEPTANCE_PACKAGED").as_deref() == Ok("1");
    // The dump exists so a gate compares the real resolver's argv. A packaged
    // dump therefore names the installed version this host must resolve, taken
    // from the gate's environment rather than from a constant in this file.
    if packaged {
        bind_packaged_session_version(
            &std::env::var("NIOH3_ACCEPTANCE_GAME_FILE_VERSION")
                .expect("a packaged dump must name the game file version under test"),
        );
    }
    let state = std::env::var("NIOH3_ACCEPTANCE_STATE_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("nioh3-acceptance-state"));
    let (executable, arguments) = resolve_role_launch(&root, &role, packaged, &state)
        .unwrap_or_else(|error| panic!("resolve failed: {error}"));
    println!(
        "NIOH3_LAUNCH {}",
        json!({
            "root": root.display().to_string(),
            "role": role,
            "packaged": packaged,
            "stateRoot": state.display().to_string(),
            "executable": executable.display().to_string(),
            "argv": arguments,
        })
    );
}

/// The shipped Python search worker takes the same identity flag and value as
/// the Rust replacement, so one session's version reaches every launch shape.
#[test]
fn the_python_search_launch_carries_the_same_resolved_game_file_version() {
    use crate::worker::{launch_command, ProtectedBackend, RustSearchEnv, SearchBackend};
    let _guard = session_version_guard();
    bind_packaged_session_version("0.7.5.0");
    let root = staged_rust_package("python-identity");
    let state = std::env::temp_dir().join("nioh3-python-identity-state");
    // The Python shape is the fallback graph: neither backend selects Rust.
    let search = SearchBackend::Python;
    let protected = ProtectedBackend::Python;

    // Packaged: the shipped worker EXE gets the session's exact version.
    let (executable, arguments) =
        launch_command(&root, "offline_search", true, &search, &protected, &state);
    assert_eq!(
        executable,
        root.join("worker/nioh3-search-worker.exe"),
        "packaged Python search runs the shipped EXE"
    );
    let index = arguments
        .iter()
        .position(|token| token == "--game-file-version")
        .unwrap_or_else(|| panic!("packaged Python argv lacks the version: {arguments:?}"));
    assert_eq!(arguments[index + 1], "0.7.5.0");
    assert!(
        !arguments
            .iter()
            .any(|token| token == "--legacy-test-context"),
        "packaged production must never select the legacy identity: {arguments:?}"
    );
    assert!(
        !arguments.iter().any(|token| token.contains("UNAVAILABLE")),
        "a resolved session must not report an unavailable version: {arguments:?}"
    );
    // The packaged acceptance entry point resolves the same way, so the argv the
    // gate observes is the argv the host would spawn for the Python graph too.
    let (_, gate_arguments) =
        crate::worker::resolve_role_launch(&root, "offline_search", true, &state)
            .expect("the packaged Python search role must resolve");
    let gate_index = gate_arguments
        .iter()
        .position(|token| token == "--game-file-version")
        .unwrap_or_else(|| panic!("gate argv lacks the version: {gate_arguments:?}"));
    assert_eq!(gate_arguments[gate_index + 1], "0.7.5.0");

    // A packaged protected role keeps its own shipped shape and no version flag.
    let (protected_executable, protected_arguments) =
        launch_command(&root, "save", true, &search, &protected, &state);
    assert_eq!(
        protected_executable,
        root.join("worker/nioh3-protected-worker.exe")
    );
    assert_eq!(protected_arguments[0], "--role");
    assert_eq!(protected_arguments[1], "save");
    assert!(
        !protected_arguments
            .iter()
            .any(|token| token == "--legacy-test-context"),
        "packaged production must never select the legacy identity"
    );
    let _ = std::fs::remove_dir_all(&root);

    // Development follows the explicit environment selection. The launcher reads
    // process env, so drive it through the same accessor the host uses.
    let _ = RustSearchEnv::default();
    release_packaged_session_version();
}

/// The Python branch's development shape passes the explicit environment
/// version through and uses the legacy flag only when that opt-in is set.
#[test]
fn the_python_search_launch_development_shape_is_explicit() {
    use crate::worker::push_python_identity_for_test;

    // A development host reads its explicit environment selection, so the test
    // drives the accessor the resolver uses rather than a copy of its rules.
    let env = RustSearchEnv::default();
    let mut versioned: Vec<String> = Vec::new();
    push_python_identity_for_test(&mut versioned, false, Some("2.0.1.0"), false);
    assert_eq!(
        versioned,
        vec!["--game-file-version".to_string(), "2.0.1.0".to_string()]
    );

    let mut legacy: Vec<String> = Vec::new();
    push_python_identity_for_test(&mut legacy, false, None, true);
    assert_eq!(legacy, vec!["--legacy-test-context".to_string()]);

    // A development launch with neither selection sends no identity flag; the
    // worker refuses it, which is the intended fail-closed shape.
    let mut bare: Vec<String> = Vec::new();
    push_python_identity_for_test(&mut bare, false, None, false);
    assert!(bare.is_empty(), "{bare:?}");

    // A development host does not read the packaged session value, so an unset
    // environment is not permission to invent a version or the legacy opt-in.
    assert_eq!(env.game_file_version, None);
    assert!(!env.legacy_test_context);
}
