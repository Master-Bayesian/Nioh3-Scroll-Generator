use crate::broker::Broker;
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
