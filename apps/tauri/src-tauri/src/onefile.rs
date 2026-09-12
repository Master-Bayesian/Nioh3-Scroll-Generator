use crate::{package, storage};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

const MAGIC: &[u8; 16] = b"NIOH3_ONEFILE_V1";
const FOOTER_SIZE: u64 = 56;

pub struct Context {
    pub executable: PathBuf,
    pub launcher_pid: u32,
}

fn regular_file(path: &Path) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err("ONEFILE_LINK_REFUSED".into());
        }
    }
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err("ONEFILE_FILE_INVALID".into());
    }
    Ok(())
}

pub fn payload_hash(path: &Path) -> Result<String, String> {
    regular_file(path)?;
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    if size <= FOOTER_SIZE {
        return Err("ONEFILE_FOOTER_INVALID".into());
    }
    file.seek(SeekFrom::End(-(FOOTER_SIZE as i64)))
        .map_err(|e| e.to_string())?;
    let mut footer = [0u8; FOOTER_SIZE as usize];
    file.read_exact(&mut footer).map_err(|e| e.to_string())?;
    let length = u64::from_le_bytes(footer[16..24].try_into().unwrap());
    if &footer[..16] != MAGIC || length == 0 || length > 536_870_912 || length >= size - FOOTER_SIZE
    {
        return Err("ONEFILE_FOOTER_INVALID".into());
    }
    file.seek(SeekFrom::Start(size - FOOTER_SIZE - length))
        .map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let copied = std::io::copy(&mut file.take(length), &mut hash).map_err(|e| e.to_string())?;
    let digest = hash.finalize();
    if copied != length || digest[..] != footer[24..] {
        return Err("ONEFILE_PAYLOAD_MISMATCH".into());
    }
    Ok(format!("{digest:x}"))
}

pub fn context() -> Result<Option<Context>, String> {
    let Some(outer) = std::env::var_os("NIOH3_ONEFILE_EXE") else {
        return Ok(None);
    };
    let executable = PathBuf::from(outer);
    if !executable.is_absolute() {
        return Err("ONEFILE_CONTEXT_INVALID".into());
    }
    regular_file(&executable)?;
    let executable = executable.canonicalize().map_err(|e| e.to_string())?;
    let expected =
        std::env::var("NIOH3_ONEFILE_PAYLOAD_SHA256").map_err(|_| "ONEFILE_CONTEXT_INVALID")?;
    if payload_hash(&executable)? != expected {
        return Err("ONEFILE_CONTEXT_MISMATCH".into());
    }
    let cache = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("ONEFILE_CONTEXT_INVALID")?)
        .join("Nioh3Studio/onefile")
        .join(&expected)
        .join(package::ENTRY)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if cache
        != std::env::current_exe()
            .map_err(|e| e.to_string())?
            .canonicalize()
            .map_err(|e| e.to_string())?
    {
        return Err("ONEFILE_RUNTIME_MISMATCH".into());
    }
    let launcher_pid = std::env::var("NIOH3_ONEFILE_PID")
        .map_err(|_| "ONEFILE_CONTEXT_INVALID")?
        .parse::<u32>()
        .map_err(|_| "ONEFILE_CONTEXT_INVALID")?;
    if launcher_pid == 0 || launcher_pid == std::process::id() {
        return Err("ONEFILE_CONTEXT_INVALID".into());
    }
    Ok(Some(Context {
        executable,
        launcher_pid,
    }))
}

// Only call after signature, archive digest, and extracted manifest verification.
pub fn assemble(
    stage: &Path,
    archive: &Path,
    destination: &Path,
    expected_zip_hash: &str,
) -> Result<String, String> {
    if expected_zip_hash.len() != 64 || !expected_zip_hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("ONEFILE_ARCHIVE_HASH_INVALID".into());
    }
    let manifest = package::verify(stage)?;
    let stub_name = "launcher/Nioh3Launcher.exe";
    let stub_entry = manifest
        .files
        .iter()
        .find(|entry| entry.path == stub_name)
        .ok_or("ONEFILE_LAUNCHER_MISSING")?;
    regular_file(archive)?;
    if package::hash_file(archive)? != expected_zip_hash {
        return Err("ONEFILE_ARCHIVE_MISMATCH".into());
    }
    let mut stub = File::open(stage.join(stub_name)).map_err(|e| e.to_string())?;
    let mut zip = File::open(archive).map_err(|e| e.to_string())?;
    let length = zip.metadata().map_err(|e| e.to_string())?.len();
    let mut output = File::options()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|e| e.to_string())?;
    let mut stub_digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = stub.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        stub_digest.update(&buffer[..count]);
        output
            .write_all(&buffer[..count])
            .map_err(|e| e.to_string())?;
    }
    if format!("{:x}", stub_digest.finalize()) != stub_entry.sha256 {
        return Err("ONEFILE_LAUNCHER_CHANGED".into());
    }
    std::io::copy(&mut zip, &mut output).map_err(|e| e.to_string())?;
    output.write_all(MAGIC).map_err(|e| e.to_string())?;
    output
        .write_all(&length.to_le_bytes())
        .map_err(|e| e.to_string())?;
    let digest = (0..32)
        .map(|n| u8::from_str_radix(&expected_zip_hash[n * 2..n * 2 + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    output.write_all(&digest).map_err(|e| e.to_string())?;
    output.sync_all().map_err(|e| e.to_string())?;
    drop(output);
    if payload_hash(destination)? != expected_zip_hash {
        return Err("ONEFILE_ASSEMBLY_MISMATCH".into());
    }
    package::hash_file(destination)
}

pub fn acknowledge(
    receipt: &mut Value,
    report: &Path,
    runtime: &Path,
    context: &Context,
) -> Result<(), String> {
    let target = PathBuf::from(receipt["target"].as_str().ok_or("UPDATE_RECEIPT_INVALID")?);
    regular_file(&target)?;
    if target.canonicalize().map_err(|e| e.to_string())? != context.executable
        || receipt["fileHash"].as_str() != Some(package::hash_file(&target)?.as_str())
        || receipt["manifestHash"].as_str()
            != Some(package::hash_file(&runtime.join("build-manifest.json"))?.as_str())
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
        || previous
            .parent()
            .ok_or("UPDATE_RECEIPT_INVALID")?
            .canonicalize()
            .map_err(|e| e.to_string())?
            != target
                .parent()
                .ok_or("UPDATE_TARGET_INVALID")?
                .canonicalize()
                .map_err(|e| e.to_string())?
    {
        return Err("UPDATE_PREVIOUS_INVALID".into());
    }
    if previous.exists() {
        regular_file(&previous)?;
        if receipt["previousHash"].as_str() != Some(package::hash_file(&previous)?.as_str()) {
            return Err("UPDATE_PREVIOUS_MISMATCH".into());
        }
        std::fs::remove_file(previous).map_err(|e| e.to_string())?;
    }
    receipt["status"] = json!("completed");
    storage::write_json(report, receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(windows)]
    fn real_onefile_helper_refuses_changed_download_without_touching_original() {
        use std::os::windows::process::CommandExt;
        let root = std::env::temp_dir().join(format!("onefile-refuse-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("Studio.exe");
        let stage = root.join("replacement.exe");
        let helper = root.join("apply-onefile-update.ps1");
        std::fs::write(&target, b"original application").unwrap();
        std::fs::write(&stage, b"changed download").unwrap();
        std::fs::write(&helper, include_str!("../apply-onefile-update.ps1")).unwrap();
        let output = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(&helper)
            .args([
                "-ProcessId",
                "2147483647",
                "-LauncherProcessId",
                "2147483646",
                "-Target",
            ])
            .arg(&target)
            .arg("-Staged")
            .arg(&stage)
            .arg("-FileHash")
            .arg("a".repeat(64))
            .arg("-PreviousHash")
            .arg(package::hash_file(&target).unwrap())
            .arg("-ManifestHash")
            .arg("b".repeat(64))
            .arg("-Profile")
            .arg(&root)
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(std::fs::read(&target).unwrap(), b"original application");
        let receipt: Value =
            serde_json::from_slice(&std::fs::read(root.join("last-update-result.json")).unwrap())
                .unwrap();
        assert_eq!(receipt["status"], "failed");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 4);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[cfg(windows)]
    fn real_onefile_helper_rechecks_prepared_bytes_after_waiting_for_exit() {
        use std::os::windows::process::CommandExt;
        let root = std::env::temp_dir().join(format!("onefile-wait-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("Studio.exe");
        let stage = root.join("replacement.exe");
        let helper = root.join("apply-onefile-update.ps1");
        std::fs::write(&target, b"original application").unwrap();
        std::fs::write(&stage, b"verified replacement").unwrap();
        std::fs::write(&helper, include_str!("../apply-onefile-update.ps1")).unwrap();
        let mut original_process = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 20",
            ])
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        let mut helper_process = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(&helper)
            .arg("-ProcessId")
            .arg(original_process.id().to_string())
            .args(["-LauncherProcessId", "2147483646", "-Target"])
            .arg(&target)
            .arg("-Staged")
            .arg(&stage)
            .arg("-FileHash")
            .arg(package::hash_file(&stage).unwrap())
            .arg("-PreviousHash")
            .arg(package::hash_file(&target).unwrap())
            .arg("-ManifestHash")
            .arg("b".repeat(64))
            .arg("-Profile")
            .arg(&root)
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        let mut changed = false;
        for _ in 0..200 {
            let prepared = std::fs::read_dir(&root)
                .unwrap()
                .filter_map(Result::ok)
                .find(|p| {
                    p.file_name()
                        .to_string_lossy()
                        .starts_with("Studio.exe.update-")
                });
            if let Some(prepared) = prepared {
                // Let the helper enter Wait-Process after its initial digest check.
                std::thread::sleep(std::time::Duration::from_millis(300));
                std::fs::write(prepared.path(), b"changed during exit wait").unwrap();
                changed = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        original_process.kill().unwrap();
        original_process.wait().unwrap();
        let status = helper_process.wait().unwrap();
        assert!(changed);
        assert!(!status.success());
        assert_eq!(std::fs::read(&target).unwrap(), b"original application");
        let receipt: Value =
            serde_json::from_slice(&std::fs::read(root.join("last-update-result.json")).unwrap())
                .unwrap();
        assert_eq!(receipt["error"], "Replacement changed while waiting");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 4);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn footer_rejects_truncation_length_and_payload_corruption() {
        let path = std::env::temp_dir().join(format!("onefile-{}.exe", uuid::Uuid::new_v4()));
        let zip = b"fake archive for footer verification";
        let digest = Sha256::digest(zip);
        let mut bytes = b"MZstub".to_vec();
        bytes.extend(zip);
        bytes.extend(MAGIC);
        bytes.extend((zip.len() as u64).to_le_bytes());
        bytes.extend(digest);
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(payload_hash(&path).unwrap(), format!("{digest:x}"));
        bytes[6] ^= 1;
        std::fs::write(&path, &bytes).unwrap();
        assert!(payload_hash(&path).is_err());
        std::fs::write(&path, &bytes[..30]).unwrap();
        assert!(payload_hash(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn acknowledgement_preserves_unmatched_backup_and_checks_both_executable_and_manifest() {
        let root = std::env::temp_dir().join(format!("onefile-ack-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("runtime")).unwrap();
        let target = root.join("Studio.exe");
        let previous = root.join(format!(
            "Studio.exe.previous-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let runtime = root.join("runtime");
        let report = root.join("receipt.json");
        std::fs::write(&target, b"new launcher").unwrap();
        std::fs::write(&previous, b"old launcher").unwrap();
        std::fs::write(runtime.join("build-manifest.json"), b"manifest").unwrap();
        let context = Context {
            executable: target.canonicalize().unwrap(),
            launcher_pid: 1,
        };
        let mut receipt = json!({"status":"awaiting-startup", "target":target,"previous":previous,
            "fileHash":package::hash_file(&target).unwrap(), "previousHash":"wrong",
            "manifestHash":package::hash_file(&runtime.join("build-manifest.json")).unwrap()});
        assert!(acknowledge(&mut receipt, &report, &runtime, &context).is_err());
        assert!(previous.exists());
        receipt["previousHash"] = json!(package::hash_file(&previous).unwrap());
        receipt["fileHash"] = json!("wrong");
        assert!(acknowledge(&mut receipt, &report, &runtime, &context).is_err());
        assert!(previous.exists());
        receipt["fileHash"] = json!(package::hash_file(&target).unwrap());
        acknowledge(&mut receipt, &report, &runtime, &context).unwrap();
        assert_eq!(receipt["status"], "completed");
        assert!(!previous.exists());
        assert_eq!(std::fs::read(target).unwrap(), b"new launcher");
        // This unique test directory contains only files created above.
        std::fs::remove_dir_all(root).unwrap();
    }
}
