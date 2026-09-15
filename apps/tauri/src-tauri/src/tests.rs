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

/// The development Rust worker selection never changes the shipped product and
/// never silently falls back to Python once an operator has asked for it.
#[test]
fn a_packaged_host_without_a_staged_manifest_keeps_the_shipped_search_worker() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let named = RustSearchEnv {
        executable: Some(root.join("Cargo.toml").display().to_string()),
        data_root: Some("elsewhere".to_string()),
        contract_dir: None,
        accelerator: None,
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
        contract_dir: None,
        accelerator: None,
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
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// A staged package that declares a binary or a root it does not carry is a
/// named refusal, never a silent fallback to the shipped Python worker.
#[test]
fn packaged_manifest_fails_closed_for_missing_or_escaping_declarations() {
    use crate::worker::{search_backend, staged_backend_manifest, RustSearchEnv};
    let root = staged_rust_package("packaged-refuse");
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
    let _ = std::fs::remove_dir_all(&outside);
}

/// A staged binary whose bytes no longer match the declared sha256 is refused by
/// name. The manifest is a declaration, not an oracle, so replacing the file
/// after staging must not be launchable.
#[test]
fn packaged_manifest_refuses_a_binary_that_changed_after_staging() {
    use crate::worker::{resolve_role_launch, staged_backend_manifest, verify_declared_binary};
    let root = staged_rust_package("packaged-mutation");
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
    let _ = std::fs::remove_dir_all(&root);
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
