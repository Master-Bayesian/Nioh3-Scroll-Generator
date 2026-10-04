#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Synthetic save-size rejection through the real protected job owner.
//! No installed game or user save is opened.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use nioh3_protected::{ProtectedJobs, RoleApplication, SaveApplication};
use nioh3_save::crypto::{SYSTEM_CONTAINER_BYTES, USER_CONTAINER_BYTES};
use nioh3_save::SaveReadError;
use nioh3_worker::{ContextSelection, Engine, GameFileVersion};
use serde_json::{json, Value};

struct SaveJobs {
    application: Arc<Mutex<SaveApplication>>,
    jobs: ProtectedJobs,
}

impl SaveJobs {
    fn run(&self, operation: &str, params: Value) -> Value {
        let application = Arc::clone(&self.application);
        let name = operation.to_string();
        let started = Instant::now();
        let accepted = self
            .jobs
            .start(operation, false, move |ctx| {
                application.lock().unwrap().run(&name, params, ctx)
            })
            .unwrap();
        let id = accepted["job_id"].as_str().unwrap();
        let deadline = started + Duration::from_secs(10);
        loop {
            let snapshot = self.jobs.snapshot(id).unwrap();
            if snapshot["state"] != "running" {
                self.jobs.join();
                eprintln!(
                    "save-size operation={operation} elapsed_ms={} state={} error={}",
                    started.elapsed().as_millis(),
                    snapshot["state"],
                    snapshot["error"],
                );
                return snapshot;
            }
            assert!(
                Instant::now() < deadline,
                "synthetic save-size refusal exceeded its bounded deadline"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }
}

#[test]
fn rejects_unsupported_save_sizes_before_digest_checks_and_keeps_jobs_usable() {
    // The project build root when set, otherwise Cargo's own per-target temp
    // directory (inside the target directory, never the checkout).
    let build_root = std::env::var_os("NIOH3_BUILD_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")));
    let root = build_root
        .join("tmp")
        .join(format!("nioh3-save-size-{}", std::process::id()));
    let path = root
        .join("76561198000000123")
        .join("SAVEDATA00")
        .join("SAVEDATA.BIN");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let data = repo.join("nioh3_scroll_editor/data");
    let contracts = repo.join("packages/contracts");
    let engine = Engine::load(
        &data,
        &contracts,
        None,
        ContextSelection::Production(GameFileVersion(2, 0, 2, 0)),
    )
    .unwrap();
    let host = SaveJobs {
        application: Arc::new(Mutex::new(
            SaveApplication::new(root.join("state"), &data, engine.context().clone()).unwrap(),
        )),
        jobs: ProtectedJobs::new(),
    };
    // This is a bounded synthetic file, never an allocation/OOM stress case.
    for length in [
        16 * 1024 * 1024,
        0,
        SYSTEM_CONTAINER_BYTES - 1,
        USER_CONTAINER_BYTES + 1,
    ] {
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(length as u64).unwrap();
        drop(file);
        let before = std::fs::metadata(&path).unwrap();
        let registered = host.run("register", json!({"path": path}));
        assert_eq!(registered["state"], "completed", "{registered}");
        let save_id = &registered["result"]["save_id"];
        let expected = SaveReadError::ContainerLength { actual: length }.to_string();
        for operation in ["inventory", "character", "prepare_character_edit"] {
            let result = host.run(
                operation,
                json!({
                    "save_id": save_id,
                    "source_sha256": "0".repeat(64),
                    "currencies": {"gold": 1},
                }),
            );
            assert_eq!(result["state"], "failed", "{operation}: {result}");
            assert_eq!(result["error"]["code"], "OPERATION_FAILED");
            assert_eq!(
                result["error"]["message"], expected,
                "{operation} must reject unsupported input before digest work",
            );
            let after = std::fs::metadata(&path).unwrap();
            assert_eq!(after.len(), before.len(), "refusal changed the source");
            assert_eq!(
                after.modified().unwrap(),
                before.modified().unwrap(),
                "refusal modified the source",
            );
            let usable = host.run("backup_location", json!({}));
            assert_eq!(usable["state"], "completed", "{usable}");
        }
    }
    assert!(!root.join("state/v2-operations").exists());
    std::fs::remove_dir_all(&root).unwrap();
}
