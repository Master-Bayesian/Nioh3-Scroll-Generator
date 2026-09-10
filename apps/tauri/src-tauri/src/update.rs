use crate::{package, storage};
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::Mutex;

#[derive(Clone, Deserialize, Serialize)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub schema: String,
    pub version: String,
    pub channel: String,
    pub platform: String,
    pub notes: String,
    pub asset: Asset,
    pub signature: String,
}
#[derive(Serialize)]
struct Payload<'a> {
    schema: &'a str,
    version: &'a str,
    channel: &'a str,
    platform: &'a str,
    notes: &'a str,
    asset: &'a Asset,
}
pub fn validate(manifest: &Manifest) -> Result<(), String> {
    let key = base64::engine::general_purpose::STANDARD
        .decode("c6oPCnJE4B+7ZnDUkZRJzUo3PZQmlM/eMlFqRC1h3dU=")
        .map_err(|e| e.to_string())?;
    validate_with_key(manifest, key.try_into().map_err(|_| "UPDATE_KEY_INVALID")?)
}
pub(crate) fn validate_with_key(manifest: &Manifest, key: [u8; 32]) -> Result<(), String> {
    let a = &manifest.asset;
    if manifest.schema != "nioh3-tauri-update/v1"
        || manifest.platform != "win32-x64"
        || !["stable", "beta"].contains(&manifest.channel.as_str())
        || manifest.notes.len() > 32000
        || manifest.version.len() > 40
        || version(&manifest.version).is_none()
    {
        return Err("UPDATE_MANIFEST_INVALID".into());
    }
    if manifest.channel == "stable" && manifest.version.contains('-') {
        return Err("UPDATE_CHANNEL_MISMATCH".into());
    }
    if !package::safe_relative(&a.name)
        || a.name.contains('/')
        || !a.name.ends_with(".zip")
        || a.size == 0
        || a.size > 1_073_741_824
        || a.sha256.len() != 64
        || !a.sha256.bytes().all(|c| c.is_ascii_hexdigit())
    {
        return Err("UPDATE_ASSET_INVALID".into());
    }
    let expected = format!(
        "https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/download/v{}/{}",
        manifest.version, a.name
    );
    if a.url != expected {
        return Err("UPDATE_ASSET_ORIGIN_INVALID".into());
    }
    let decode = |s: &str| {
        base64::engine::general_purpose::STANDARD
            .decode(s)
            .map_err(|e| e.to_string())
    };
    let signature =
        Signature::from_slice(&decode(&manifest.signature)?).map_err(|e| e.to_string())?;
    let payload = Payload {
        schema: &manifest.schema,
        version: &manifest.version,
        channel: &manifest.channel,
        platform: &manifest.platform,
        notes: &manifest.notes,
        asset: a,
    };
    VerifyingKey::from_bytes(&key)
        .map_err(|e| e.to_string())?
        .verify(
            &serde_json::to_vec(&payload).map_err(|e| e.to_string())?,
            &signature,
        )
        .map_err(|_| "UPDATE_SIGNATURE_INVALID".into())
}
fn version(value: &str) -> Option<(u32, u32, u32, u8, u32)> {
    let (base, suffix) = value.split_once('-').unwrap_or((value, ""));
    let parts: Vec<_> = base.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let (rank, n) = if suffix.is_empty() {
        (3, 0)
    } else {
        let (tag, n) = suffix.split_once('.')?;
        (
            match tag {
                "beta" => 1,
                "rc" => 2,
                _ => return None,
            },
            n.parse().ok()?,
        )
    };
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
        rank,
        n,
    ))
}
pub struct Updater {
    pub root: PathBuf,
    state: Mutex<Value>,
    manifest: Mutex<Option<Manifest>>,
    pub ready: Mutex<Option<(PathBuf, String)>>,
}
impl Updater {
    pub fn new(root: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            root,
            state: Mutex::new(json!({"phase":"idle"})),
            manifest: Mutex::new(None),
            ready: Mutex::new(None),
        })
    }
    pub async fn state(&self, can_apply: bool) -> Value {
        let mut value = self.state.lock().await.clone();
        value["canApply"] = json!(can_apply);
        value
    }
    pub async fn action(self: &Arc<Self>, action: &str, channel: &str) -> Result<(), String> {
        if !["stable", "beta"].contains(&channel) {
            return Err("INVALID_UPDATE_CHANNEL".into());
        }
        let mut state = self.state.lock().await;
        if ["checking", "downloading", "ready"].contains(&state["phase"].as_str().unwrap_or("")) {
            return Ok(());
        }
        if action == "check" {
            *state = json!({"phase":"checking"});
            let updater = self.clone();
            let channel = channel.to_string();
            tokio::spawn(async move {
                if let Err(e) = updater.check(&channel).await {
                    *updater.state.lock().await = json!({"phase":"failed","error":e});
                }
            });
            Ok(())
        } else if action == "download" && state["phase"] == "available" {
            *state = json!({"phase":"downloading","downloaded":0});
            let updater = self.clone();
            tokio::spawn(async move {
                if let Err(e) = updater.download().await {
                    *updater.state.lock().await = json!({"phase":"failed","error":e});
                }
            });
            Ok(())
        } else {
            Err("INVALID_UPDATE_ACTION".into())
        }
    }
    fn client() -> Result<reqwest::Client, String> {
        reqwest::Client::builder()
            .user_agent("Nioh3Studio")
            .timeout(Duration::from_secs(180))
            .build()
            .map_err(|e| e.to_string())
    }
    async fn bounded(client: &reqwest::Client, url: &str, limit: usize) -> Result<Vec<u8>, String> {
        let mut response = client
            .get(url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        let mut bytes = vec![];
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            if bytes.len() + chunk.len() > limit {
                return Err("UPDATE_RESPONSE_TOO_LARGE".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
    async fn check(&self, channel: &str) -> Result<(), String> {
        let client = Self::client()?;
        let releases:Vec<Value>=serde_json::from_slice(&Self::bounded(&client,"https://api.github.com/repos/Master-Bayesian/Nioh3-Scroll-Generator/releases?per_page=20",1_048_576).await?).map_err(|e|e.to_string())?;
        let mut best: Option<Manifest> = None;
        for release in releases.iter().take(20) {
            if release["draft"] == true || (channel == "stable" && release["prerelease"] == true) {
                continue;
            }
            let Some(assets) = release["assets"].as_array() else {
                continue;
            };
            let Some(asset) = assets.iter().find(|a| a["name"] == "tauri-update.json") else {
                continue;
            };
            let url = asset["browser_download_url"]
                .as_str()
                .ok_or("UPDATE_ORIGIN_INVALID")?;
            let prefix =
                "https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/download/";
            if !url.starts_with(prefix) {
                return Err("UPDATE_ORIGIN_INVALID".into());
            }
            let manifest: Manifest =
                serde_json::from_slice(&Self::bounded(&client, url, 131072).await?)
                    .map_err(|e| e.to_string())?;
            validate(&manifest)?;
            if manifest.channel == channel
                && best
                    .as_ref()
                    .is_none_or(|b| version(&manifest.version) > version(&b.version))
            {
                best = Some(manifest);
            }
        }
        if let Some(manifest) =
            best.filter(|m| version(&m.version) > version(env!("CARGO_PKG_VERSION")))
        {
            *self.manifest.lock().await = Some(manifest.clone());
            *self.state.lock().await =
                json!({"phase":"available","version":manifest.version,"notes":manifest.notes});
        } else {
            *self.state.lock().await = json!({"phase":"current"});
        }
        Ok(())
    }
    async fn download(&self) -> Result<(), String> {
        let manifest = self
            .manifest
            .lock()
            .await
            .clone()
            .ok_or("UPDATE_NOT_AVAILABLE")?;
        validate(&manifest)?;
        let folder = self.root.join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let result=async{
            let archive=folder.join("package.zip");let stage=folder.join("package");
            let mut response=Self::client()?.get(&manifest.asset.url).timeout(Duration::from_secs(600)).send().await.map_err(|e|e.to_string())?.error_for_status().map_err(|e|e.to_string())?;
            let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(&archive).map_err(|e|e.to_string())?;
            let mut size=0u64;let mut hash=Sha256::new();
            while let Some(chunk)=response.chunk().await.map_err(|e|e.to_string())?{
                size+=chunk.len()as u64;if size>manifest.asset.size{return Err("UPDATE_TOO_LARGE".into());}hash.update(&chunk);file.write_all(&chunk).map_err(|e|e.to_string())?;
                *self.state.lock().await=json!({"phase":"downloading","version":manifest.version,"downloaded":size,"total":manifest.asset.size});
            }
            file.sync_all().map_err(|e|e.to_string())?;drop(file);
            if size!=manifest.asset.size||format!("{:x}",hash.finalize())!=manifest.asset.sha256{return Err("UPDATE_HASH_MISMATCH".into());}
            package::extract(&archive,&stage)?;
            if package::verify(&stage)?.version!=manifest.version{return Err("UPDATE_VERSION_MISMATCH".into());}
            storage::write_json(&folder.join("verified-update.json"),&serde_json::to_value(&manifest).map_err(|e|e.to_string())?)?;
            std::fs::remove_file(archive).map_err(|e|e.to_string())?;
            let hash=package::hash_file(&stage.join("build-manifest.json"))?;
            *self.ready.lock().await=Some((stage,hash));*self.state.lock().await=json!({"phase":"ready","version":manifest.version});Ok(())
        }.await;
        if result.is_err() {
            let _ = package::remove_child(&self.root, &folder, false);
        }
        result
    }
    pub fn cleanup(&self, target: &Path) -> Result<(), String> {
        if !self.root.exists() {
            return Ok(());
        }
        let report = self.root.join("last-update-result.json");
        if report.exists() {
            let mut receipt: Value =
                serde_json::from_slice(&std::fs::read(&report).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            if receipt["status"] == "awaiting-startup" {
                let recorded =
                    PathBuf::from(receipt["target"].as_str().ok_or("UPDATE_RECEIPT_INVALID")?)
                        .canonicalize()
                        .map_err(|e| e.to_string())?;
                if recorded != target.canonicalize().map_err(|e| e.to_string())?
                    || receipt["manifestHash"].as_str().map(str::to_lowercase)
                        != Some(package::hash_file(&target.join("build-manifest.json"))?)
                {
                    return Err("UPDATE_RECEIPT_MISMATCH".into());
                }
                let previous = PathBuf::from(
                    receipt["previous"]
                        .as_str()
                        .ok_or("UPDATE_RECEIPT_INVALID")?,
                );
                let expected = format!(
                    "{}.previous-",
                    target
                        .file_name()
                        .ok_or("UPDATE_TARGET_INVALID")?
                        .to_string_lossy()
                );
                let name = previous
                    .file_name()
                    .ok_or("UPDATE_RECEIPT_INVALID")?
                    .to_string_lossy();
                if !name.starts_with(&expected)
                    || uuid::Uuid::parse_str(&name[expected.len()..]).is_err()
                {
                    return Err("UPDATE_PREVIOUS_INVALID".into());
                }
                // The NSIS installer owns one file beside the portable
                // product. The update helper copies it into the replacement so
                // Add/Remove Programs keeps working. Remove the rollback copy
                // only when it is byte-identical to the installed copy; every
                // other extra file still causes fail-safe preservation below.
                let previous_uninstaller = previous.join("uninstall.exe");
                if previous_uninstaller.exists() {
                    let installed_uninstaller = target.join("uninstall.exe");
                    let previous_metadata = std::fs::symlink_metadata(&previous_uninstaller)
                        .map_err(|e| e.to_string())?;
                    let installed_metadata = std::fs::symlink_metadata(&installed_uninstaller)
                        .map_err(|e| e.to_string())?;
                    if previous_metadata.file_type().is_symlink()
                        || installed_metadata.file_type().is_symlink()
                        || !previous_metadata.is_file()
                        || !installed_metadata.is_file()
                        || previous_metadata.len() == 0
                        || previous_metadata.len() > 64 * 1024 * 1024
                        || previous_metadata.len() != installed_metadata.len()
                        || package::hash_file(&previous_uninstaller)?
                            != package::hash_file(&installed_uninstaller)?
                    {
                        return Err("UPDATE_UNINSTALLER_MISMATCH".into());
                    }
                    std::fs::remove_file(previous_uninstaller).map_err(|e| e.to_string())?;
                }
                package::remove_child(
                    target.parent().ok_or("UPDATE_TARGET_INVALID")?,
                    &previous,
                    true,
                )?;
                receipt["status"] = json!("completed");
                storage::write_json(&report, &receipt)?;
            } else if receipt["status"] != "completed" && receipt["status"] != "failed" {
                return Ok(());
            }
        }
        for entry in std::fs::read_dir(&self.root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if uuid::Uuid::parse_str(&entry.file_name().to_string_lossy()).is_ok() {
                package::remove_child(&self.root, &entry.path(), false)?;
            }
        }
        Ok(())
    }
    pub async fn launch(&self, target: &Path) -> Result<(), String> {
        let (stage, hash) = self.ready.lock().await.clone().ok_or("UPDATE_NOT_READY")?;
        package::verify(&stage)?;
        let helper = self.root.join("apply-update.ps1");
        std::fs::write(&helper, include_str!("../apply-update.ps1")).map_err(|e| e.to_string())?;
        let mut command = std::process::Command::new("powershell.exe");
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(helper)
            .args(["-ProcessId", &std::process::id().to_string(), "-Target"])
            .arg(target)
            .arg("-Staged")
            .arg(stage)
            .arg("-ManifestHash")
            .arg(hash)
            .arg("-Profile")
            .arg(self.root.parent().ok_or("UPDATE_PROFILE_INVALID")?)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        command.spawn().map_err(|e| e.to_string())?;
        Ok(())
    }
}
