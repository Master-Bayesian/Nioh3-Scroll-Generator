use crate::worker::{Reply, Worker};
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

pub struct Broker {
    pub root: PathBuf,
    pub data: PathBuf,
    packaged: bool,
    workers: Mutex<HashMap<String, Arc<Worker>>>,
    records: Mutex<HashMap<String, Value>>,
    submitted: Mutex<Value>,
    pub storage: Mutex<()>,
}
impl Broker {
    pub async fn diagnostics(&self) -> Value {
        let hosts: Vec<_> = self.workers.lock().await.values().cloned().collect();
        let mut workers = vec![];
        for host in hosts {
            workers.push(host.diagnostics().await);
        }
        json!(workers)
    }
    pub fn new(root: PathBuf, data: PathBuf, packaged: bool) -> Self {
        Self {
            root,
            data,
            packaged,
            workers: Mutex::new(HashMap::new()),
            records: Mutex::new(HashMap::new()),
            submitted: Mutex::new(Value::Null),
            storage: Mutex::new(()),
        }
    }
    pub async fn host(&self, role: &str) -> Result<Arc<Worker>, String> {
        if !["offline_search", "save", "runtime"].contains(&role) {
            return Err("INVALID_ROLE".into());
        }
        let mut hosts = self.workers.lock().await;
        if let Some(worker) = hosts.get(role) {
            return Ok(worker.clone());
        }
        let executable = if self.packaged {
            self.root.join(if role == "offline_search" {
                "worker/nioh3-search-worker.exe"
            } else {
                "worker/nioh3-protected-worker.exe"
            })
        } else {
            PathBuf::from(std::env::var("NIOH3_PYTHON").unwrap_or_else(|_| "python".into()))
        };
        let worker = Worker::spawn(
            &self.root,
            &executable,
            role,
            self.packaged,
            self.data.clone(),
        )
        .await?;
        hosts.insert(role.into(), worker.clone());
        Ok(worker)
    }
    pub async fn call(&self, role: &str, method: &str, params: Value) -> Reply {
        self.host(role).await?.call(method, params).await
    }
    pub async fn run(&self, role: &str, method: &str, params: Value) -> Reply {
        self.host(role).await?.run(method, params).await
    }
    async fn retain(&self, transfer: Value) -> Reply {
        let key = transfer["candidate_id"]
            .as_str()
            .ok_or("CANDIDATE_TRANSFER_EXPECTED")?
            .to_string();
        if !transfer["record_hex"].is_string() {
            return Err("CANDIDATE_TRANSFER_EXPECTED".into());
        }
        let mut records = self.records.lock().await;
        if records.len() >= 350 && !records.contains_key(&key) {
            return Err("CART_CAPACITY_REACHED".into());
        }
        records.insert(key.clone(), transfer);
        Ok(json!({"reference_id":key}))
    }
    async fn resolve_records(&self, values: &Value) -> Result<Vec<Value>, String> {
        let keys = values.as_array().ok_or("INVALID_CART_SELECTION")?;
        if keys.is_empty() || keys.len() > 50 {
            return Err("INVALID_CART_SELECTION".into());
        }
        let records = self.records.lock().await;
        let mut used = std::collections::HashSet::new();
        keys.iter()
            .map(|key| {
                let key = key.as_str().ok_or("INVALID_CART_SELECTION")?;
                if !used.insert(key) {
                    return Err("INVALID_CART_SELECTION".into());
                }
                records
                    .get(key)
                    .cloned()
                    .ok_or_else(|| "CART_REFERENCE_EXPIRED".into())
            })
            .collect()
    }
    async fn candidate(&self, p: &Value) -> Reply {
        match p["source"].as_str().unwrap_or("search") {
            "search" => {
                self.call(
                    "offline_search",
                    "candidate.export",
                    json!({"job_id":p["job_id"],"candidate_id":p["candidate_id"]}),
                )
                .await
            }
            "runtime" => {
                self.run(
                    "runtime",
                    "runtime.export",
                    json!({"candidate_id":p["candidate_id"]}),
                )
                .await
            }
            _ => Err("INVALID_CANDIDATE_SOURCE".into()),
        }
    }
    pub async fn dispatch(&self, channel: &str, p: Value) -> Reply {
        match channel {
            "core:handshake" => self.host("offline_search").await?.handshake().await,
            "core:catalog" => {
                self.call(
                    "offline_search",
                    "search.catalog",
                    json!({"playthrough":3,"rarity":p["rarity"],"locale":p["locale"]}),
                )
                .await
            }
            "core:recommended-level" => {
                self.call(
                    "offline_search",
                    "recommended_level.resolve",
                    json!({"displayed_level":p}),
                )
                .await
            }
            "core:start" => {
                let job = self
                    .call("offline_search", "search.start", p.clone())
                    .await?;
                *self.submitted.lock().await = json!({"job_id":job["job_id"],"params":p});
                Ok(job)
            }
            "core:current" => {
                let value = self
                    .call("offline_search", "job.current", json!({}))
                    .await?;
                let submitted = self.submitted.lock().await;
                Ok(
                    json!({"job":value["job"],"submitted":if !value["job"].is_null() && value["job"]["job_id"] == submitted["job_id"] { submitted["params"].clone() } else {Value::Null}}),
                )
            }
            "core:snapshot" | "core:cancel" => {
                self.call(
                    "offline_search",
                    if channel.ends_with("cancel") {
                        "job.cancel"
                    } else {
                        "job.snapshot"
                    },
                    json!({"job_id":p}),
                )
                .await
            }
            "core:restart" => {
                if let Some(worker) = self.workers.lock().await.remove("offline_search") {
                    worker.close().await;
                }
                *self.submitted.lock().await = Value::Null;
                self.host("offline_search").await?.handshake().await
            }
            "operations:execute" => {
                let method = p["method"].as_str().ok_or("PRIVATE_OR_UNKNOWN_OPERATION")?;
                const PUBLIC: &[&str] = &[
                    "runtime.count_execute",
                    "runtime.count_status",
                    "runtime.count_recover",
                    "save.recycle_backups",
                    "save.discover",
                    "save.inventory",
                    "save.prepare_edit",
                    "save.prepare_delete",
                    "save.backups",
                    "save.prepare_restore",
                    "save.discard",
                    "save.commit",
                    "save.operation",
                    "save.operations",
                    "runtime.status",
                    "runtime.start_override",
                    "runtime.stop_override",
                    "runtime.live_batch_execute",
                    "runtime.live_batch_status",
                    "runtime.live_batch_cancel",
                    "runtime.live_add_execute",
                    "runtime.live_add_status",
                    "runtime.live_add_recover",
                    "runtime.live_add_cancel",
                ];
                if !PUBLIC.contains(&method) {
                    return Err("PRIVATE_OR_UNKNOWN_OPERATION".into());
                }
                self.call(
                    if method.starts_with("save.") {
                        "save"
                    } else {
                        "runtime"
                    },
                    method,
                    p["params"].clone(),
                )
                .await
            }
            "operations:prepare-count" => {
                if !p["new_count"].as_u64().is_some_and(|v| v <= 7) {
                    return Err("INVALID_COUNT".into());
                }
                let source = self.run("save", "save.count_edit_source", json!({"save_id":p["save_id"],"snapshot_id":p["snapshot_id"],"slot_index":p["slot_index"]})).await?;
                self.call(
                    "runtime",
                    "runtime.count_prepare",
                    json!({"source":source["count_source"],"new_count":p["new_count"]}),
                )
                .await
            }
            "operations:current" => {
                let role = p.as_str().ok_or("INVALID_ROLE")?;
                if !["save", "runtime"].contains(&role) {
                    return Err("INVALID_ROLE".into());
                }
                let host = self.workers.lock().await.get(role).cloned();
                match host {
                    None => Ok(json!({"job":null,"busy":false})),
                    Some(host) => {
                        let value = host.call("job.current", json!({})).await?;
                        public_current(&value["job"])
                    }
                }
            }
            "operations:snapshot" | "operations:cancel" => {
                let role = p["role"].as_str().ok_or("INVALID_ROLE")?;
                if !["save", "runtime"].contains(&role) {
                    return Err("INVALID_ROLE".into());
                }
                let snapshot = self
                    .call(role, "job.snapshot", json!({"job_id":p["jobId"]}))
                    .await?;
                require_public(&snapshot)?;
                if channel.ends_with("cancel") {
                    let job = self
                        .call(role, "job.cancel", json!({"job_id":p["jobId"]}))
                        .await?;
                    require_public(&job)?;
                    Ok(job)
                } else {
                    Ok(snapshot)
                }
            }
            "review:retain" => self.retain(self.candidate(&p).await?).await,
            "review:favorites" => self.favorites(p).await,
            "review:release" => {
                self.records
                    .lock()
                    .await
                    .remove(p.as_str().ok_or("INVALID_REFERENCE")?);
                Ok(Value::Null)
            }
            "review:preview" => {
                let value = self
                    .call(
                        "offline_search",
                        "candidate.preview",
                        json!({"seed":p["seed"],"rarity":p["rarity"],"level":p["level"]}),
                    )
                    .await?;
                let reference = if p["retain"] == false {
                    Value::Null
                } else {
                    self.retain(value["transfer"].clone()).await?["reference_id"].clone()
                };
                Ok(json!({"candidate":value["candidate"],"reference_id":reference}))
            }
            "review:prepare-cart" => {
                let candidates = self.resolve_records(&p["references"]).await?;
                let request = json!({"save_id":p["save_id"],"snapshot_id":p["snapshot_id"],"candidates":candidates,"recommended_level":p["recommended_level"],"transfer_count":p["transfer_count"]});
                match p["mode"].as_str() {
                    Some("save") => {
                        self.call("save", "save.prepare_install_many", request)
                            .await
                    }
                    Some("live") => {
                        let value = self
                            .run("save", "save.materialize_live_many", request)
                            .await?;
                        self.call("runtime", "runtime.live_batch_prepare", json!({"candidates":value["candidates"],"save_path":value["save_path"]})).await
                    }
                    _ => Err("INVALID_ADDITION_MODE".into()),
                }
            }
            "review:auxiliary" => {
                let value = self.run("save", "save.auxiliary_preview", p).await?;
                serde_json::from_str(
                    value["auxiliary_json"]
                        .as_str()
                        .ok_or("AUXILIARY_EXPECTED")?,
                )
                .map_err(|e| e.to_string())
            }
            "operations:install" | "operations:live-add" => {
                let candidate = self.candidate(&p).await?;
                if channel.ends_with("install") {
                    self.call("save", "save.prepare_install", json!({"save_id":p["save_id"],"snapshot_id":p["snapshot_id"],"candidate":candidate,"recommended_level":p["recommended_level"],"transfer_count":p["transfer_count"]})).await
                } else {
                    let source = self
                        .run(
                            "save",
                            "save.live_add_source",
                            json!({"save_id":p["save_id"],"snapshot_id":p["snapshot_id"]}),
                        )
                        .await?;
                    self.call(
                        "runtime",
                        "runtime.live_add_prepare",
                        json!({"candidate":candidate,"save_path":source["save_path"]}),
                    )
                    .await
                }
            }
            "operations:generate" | "operations:native-search" | "operations:capture-grace" => {
                let template = self.run("save", "save.template", json!({"save_id":p["save_id"],"snapshot_id":p["snapshot_id"],"playthrough":p["playthrough"]})).await?;
                let mut params = json!({"template":template,"playthrough":p["playthrough"],"rarity":p["rarity"],"level":p["level"],"recommended_level":p["recommended_level"],"title_screen_confirmed":p["title_screen_confirmed"]});
                let method = if channel.ends_with("capture-grace") {
                    "runtime.capture_grace"
                } else {
                    params["seed"] = p["seed"].clone();
                    if channel.ends_with("native-search") {
                        params["criteria"] = p["criteria"].clone();
                        params["max_seeds"] = p["max_seeds"].clone();
                        params["after_trial"] = p.get("after_trial").cloned().unwrap_or(json!(0));
                        "runtime.search"
                    } else {
                        "runtime.generate"
                    }
                };
                self.call("runtime", method, params).await
            }
            "operations:bind-cache" => {
                let value = self.run("save", "save.cached_grace", p).await?;
                self.call(
                    "offline_search",
                    "cache.register",
                    json!({"cache_json":value["cache_json"]}),
                )
                .await
            }
            _ => Err(format!("UNKNOWN_DESKTOP_COMMAND: {channel}")),
        }
    }
    pub async fn shutdown(&self) -> bool {
        let hosts = self.workers.lock().await.clone();
        for role in ["save", "runtime", "offline_search"] {
            if let Some(host) = hosts.get(role) {
                if !host.close().await {
                    return false;
                }
            }
        }
        true
    }
    async fn favorites(&self, params: Value) -> Reply {
        let _guard = self.storage.lock().await;
        let path = self.data.join("favorites.json");
        let mut entries = match std::fs::metadata(&path) {
            Ok(metadata) => {
                if metadata.len() > 4_000_000 {
                    return Err("FAVORITES_FILE_INVALID".into());
                }
                let value: Value =
                    serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?;
                if value["version"] != 1 {
                    return Err("FAVORITES_FILE_INVALID".into());
                }
                value["entries"]
                    .as_array()
                    .filter(|a| a.len() <= 50)
                    .ok_or("FAVORITES_FILE_INVALID")?
                    .clone()
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => vec![],
            Err(error) => return Err(error.to_string()),
        };
        for entry in &entries {
            validate_favorite(entry)?;
        }
        match params["action"].as_str() {
            Some("list") => {}
            Some("add") => {
                let transfer = self
                    .resolve_records(&json!([params["reference_id"]]))
                    .await?
                    .remove(0);
                let mut sample = params["sample"].clone();
                if let Some(object) = sample.as_object_mut() {
                    object.remove("backend");
                    object.remove("saveEntry");
                }
                let entry = json!({"sample":sample,"transfer":transfer});
                validate_favorite(&entry)?;
                let key = sample_key(&sample);
                if let Some(index) = entries.iter().position(|e| sample_key(&e["sample"]) == key) {
                    entries[index] = entry;
                } else {
                    if entries.len() >= 50 {
                        return Err("FAVORITES_CAPACITY_REACHED".into());
                    }
                    entries.push(entry);
                }
                crate::storage::write_json(&path, &json!({"version":1,"entries":entries}))?;
            }
            Some("remove") => {
                let key = params["key"].as_str().ok_or("INVALID_FAVORITES_KEY")?;
                entries.retain(|e| sample_key(&e["sample"]) != key);
                crate::storage::write_json(&path, &json!({"version":1,"entries":entries}))?;
            }
            _ => return Err("INVALID_FAVORITES_ACTION".into()),
        }
        let mut samples = vec![];
        for entry in entries {
            let mut sample = entry["sample"].clone();
            let transfer = &entry["transfer"];
            let reference = self.retain(transfer.clone()).await?;
            sample["backend"] = json!({"candidateId":transfer["candidate_id"],"referenceId":reference["reference_id"],"installable":true});
            samples.push(sample);
        }
        Ok(json!(samples))
    }
}
fn sample_key(sample: &Value) -> String {
    format!(
        "{}:{}:{}:{}",
        sample["playthrough"].as_u64().unwrap_or(3),
        sample["rarity"],
        sample["level"].as_u64().unwrap_or(180),
        sample["seed"].as_str().unwrap_or("")
    )
}
fn validate_favorite(entry: &Value) -> Result<(), String> {
    let sample = &entry["sample"];
    let transfer = &entry["transfer"];
    if sample["seed"].as_str() != Some(transfer["seed"].to_string().as_str())
        || sample["rarity"] != transfer["rarity"]
        || sample["playthrough"].as_u64().unwrap_or(3)
            != transfer["playthrough"].as_u64().unwrap_or(0)
        || sample["level"].as_u64().unwrap_or(180) != transfer["level"].as_u64().unwrap_or(0)
        || !transfer["candidate_id"].is_string()
        || !sample["effects"].is_array()
        || !sample["rules"].is_array()
        || !sample["enemies"].is_array()
        || entry.to_string().len() > 70000
    {
        return Err("FAVORITE_INVALID".into());
    }
    Ok(())
}
fn require_public(job: &Value) -> Result<(), String> {
    let kind = job["kind"].as_str().unwrap_or("");
    const PUBLIC: &[&str] = &[
        "runtime.count_prepare",
        "runtime.count_execute",
        "runtime.count_status",
        "runtime.count_recover",
        "save.recycle_backups",
        "save.discover",
        "save.register",
        "save.inventory",
        "save.prepare_edit",
        "save.prepare_delete",
        "save.prepare_install_many",
        "runtime.live_batch_prepare",
        "runtime.live_batch_execute",
        "runtime.live_batch_status",
        "runtime.live_batch_cancel",
        "save.prepare_install",
        "save.backups",
        "save.prepare_restore",
        "save.discard",
        "save.commit",
        "save.operation",
        "save.operations",
        "runtime.generate",
        "runtime.search",
        "runtime.capture_grace",
        "runtime.start_override",
        "runtime.stop_override",
        "runtime.live_add_prepare",
        "runtime.live_add_execute",
        "runtime.live_add_status",
        "runtime.live_add_recover",
        "runtime.live_add_cancel",
    ];
    if !PUBLIC.contains(&kind) {
        return Err("PRIVATE_OPERATION_JOB".into());
    }
    Ok(())
}
fn public_current(job: &Value) -> Reply {
    if job.is_null() {
        return Ok(json!({"job":null,"busy":false}));
    }
    let busy = job["state"] == "running" || job["state"] == "cancel_requested";
    Ok(json!({"job":if require_public(job).is_ok() { job.clone() } else {Value::Null},"busy":busy}))
}
