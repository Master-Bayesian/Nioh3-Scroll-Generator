#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! No process discovery or game/save access: only the host control plane and
//! an explicitly unsupported resource selection in an owned empty directory.
use nioh3_protected::{RoleApplication, RuntimeApplication};
use nioh3_worker::{engine::ContextSelection, GameFileVersion};
use serde_json::{json, Value};
use std::path::PathBuf;

#[test]
fn runtime_control_plane_starts_without_installed_game_or_generation_resources() {
    let root = std::env::temp_dir().join(format!("runtime-unbound-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let contracts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts");
    let mut app = RuntimeApplication::deferred(
        root.join("state"),
        &root.join("missing-data"),
        &contracts,
        None,
        Some(ContextSelection::Production(GameFileVersion(9, 9, 9, 9))),
    )
    .unwrap();
    assert_eq!(
        app.context_payload(),
        Value::Null,
        "unloaded is explicit, not a borrowed version"
    );
    let before = app.direct("runtime.status", &json!({})).unwrap();
    let error = app
        .direct("runtime.scroll_completion_predict", &json!({}))
        .unwrap_err();
    assert_eq!(error.code, Some("RESOURCE_MISMATCH"));
    assert!(error.message.contains("restore the resource files"));
    assert_eq!(app.context_payload(), Value::Null);
    assert_eq!(
        app.direct("runtime.status", &json!({})).unwrap(),
        before,
        "a generation failure must not poison independent runtime controls"
    );
}

#[test]
fn actual_runtime_worker_handshakes_without_an_install_or_generation_data() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let root = std::env::temp_dir().join(format!("runtime-unbound-worker-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let contracts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts");
    let contract = nioh3_protected::Contract::load(&contracts).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_nioh3-protected-worker"))
        .env_remove("NIOH3_RUST_PROTECTED_WORKER")
        .args(["--role", "runtime", "--state-root"])
        .arg(root.join("state"))
        .arg("--data-root")
        .arg(root.join("missing-data"))
        .arg("--contract-dir")
        .arg(&contracts)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for (id, method, params) in [
        ("1", "handshake", json!({})),
        ("2", "runtime.status", json!({})),
        ("3", "runtime.compatibility", json!({"action":"cancel"})),
        ("4", "shutdown", json!({})),
    ] {
        let request = json!({"protocol":1,"id":id,"method":method,"params":params});
        assert!(contract.request_valid(&request));
        let bytes = serde_json::to_vec(&request).unwrap();
        input
            .write_all(&(bytes.len() as u32).to_le_bytes())
            .unwrap();
        input.write_all(&bytes).unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut bytes = output.stdout.as_slice();
    let mut replies = Vec::new();
    while !bytes.is_empty() {
        assert!(bytes.len() >= 4);
        let length = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        let reply: Value = serde_json::from_slice(&bytes[4..4 + length]).unwrap();
        assert!(contract.response_valid(&reply), "{reply}");
        assert_eq!(reply["ok"], true, "{reply}");
        replies.push(reply);
        bytes = &bytes[4 + length..];
    }
    assert_eq!(replies.len(), 4);
    assert_eq!(replies[0]["result"]["context"], Value::Null);
    assert_eq!(replies[1]["result"]["safe_to_shutdown"], true);
    assert_eq!(replies[2]["result"]["compatibility"]["cancelled"], true);
}
