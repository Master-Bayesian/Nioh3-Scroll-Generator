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
pub struct Worker {
    role: String,
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
            "contextDigest":context,"contractDigest":self.contract_digest,"pendingRequests":pending})
    }
    pub async fn spawn(
        root: &Path,
        executable: &Path,
        role: &str,
        packaged: bool,
        data: std::path::PathBuf,
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
        let mut command = Command::new(executable);
        if !packaged {
            command.args([
                "-u",
                "-m",
                if role == "offline_search" {
                    "nioh3_scroll_editor.search_worker"
                } else {
                    "nioh3_scroll_editor.protected_worker"
                },
            ]);
        }
        if role != "offline_search" {
            command.args(["--role", role]);
        }
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
