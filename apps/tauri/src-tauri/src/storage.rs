use serde_json::Value;
use std::{io::Write, path::Path};

pub fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let temporary = path.with_extension("json.tmp");
    let mut file = std::fs::File::create(&temporary).map_err(|e| e.to_string())?;
    file.write_all(value.to_string().as_bytes())
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    std::fs::rename(temporary, path).map_err(|e| e.to_string())
}
pub fn log(root: &Path, category: &str, message: &str) {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let Ok(_guard) = LOCK.lock() else { return };
    let directory = root.join("logs");
    let _ = std::fs::create_dir_all(&directory);
    let path = directory.join("desktop.log");
    if std::fs::metadata(&path)
        .map(|m| m.len() >= 4 * 1024 * 1024)
        .unwrap_or(false)
    {
        for index in (1..=4).rev() {
            let source = if index == 1 {
                path.clone()
            } else {
                directory.join(format!("desktop.{}.log", index - 1))
            };
            let target = directory.join(format!("desktop.{index}.log"));
            if index == 4 {
                let _ = std::fs::remove_file(&target);
            }
            let _ = std::fs::rename(source, target);
        }
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let bounded: String = message.chars().take(8192).collect();
        let _ = writeln!(file, "{timestamp} [{category}] {bounded}");
    }
}
