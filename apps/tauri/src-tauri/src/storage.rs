use serde_json::Value;
use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

const LOG_FILE_LIMIT: u64 = 4 * 1024 * 1024;
const LOG_ARCHIVE_COUNT: usize = 4;
static LOG_LOCK: Mutex<()> = Mutex::new(());
// One bounded capsule for the first actionable failure of this app session.
// Persistent operation/native receipts remain the restart recovery authority.
static FIRST_FAILURE: Mutex<Vec<(PathBuf, String)>> = Mutex::new(Vec::new());

pub fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let temporary = path.with_extension("json.tmp");
    let mut file = std::fs::File::create(&temporary).map_err(|e| e.to_string())?;
    file.write_all(value.to_string().as_bytes())
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    std::fs::rename(temporary, path).map_err(|e| e.to_string())
}

fn rotate(directory: &Path) -> std::io::Result<()> {
    for index in (1..=LOG_ARCHIVE_COUNT).rev() {
        let source = directory.join(if index == 1 {
            "desktop.log".to_owned()
        } else {
            format!("desktop.{}.log", index - 1)
        });
        let target = directory.join(format!("desktop.{index}.log"));
        if index == LOG_ARCHIVE_COUNT && target.exists() {
            std::fs::remove_file(&target)?;
        }
        if source.exists() {
            std::fs::rename(source, target)?;
        }
    }
    Ok(())
}

fn append_line(directory: &Path, line: &str) -> std::io::Result<()> {
    let path = directory.join("desktop.log");
    // Check the projected BYTES for every chunk, not once before a whole payload.
    let size = match std::fs::metadata(&path) {
        Ok(m) => m.len(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        Err(e) => return Err(e),
    };
    if size.saturating_add(line.len() as u64) > LOG_FILE_LIMIT {
        rotate(directory)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(line.as_bytes())
}

pub fn log(root: &Path, category: &str, message: &str) {
    let Ok(_guard) = LOG_LOCK.lock() else { return };
    let directory = root.join("logs");
    if std::fs::create_dir_all(&directory).is_err() {
        return;
    }
    let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    if category == "worker-error"
        || message.starts_with("[automatic-failure]")
        || (category.ends_with("-stderr")
            && (message.contains("[native-failure]")
                || message.contains("[native-receipt-write-error]")
                || message.contains("[protected-failure]")))
    {
        if let Ok(mut failure) = FIRST_FAILURE.lock() {
            if !failure.iter().any(|(owner, _)| owner == root) {
                let mut end = message.len().min(16_000);
                while !message.is_char_boundary(end) {
                    end -= 1;
                }
                // The application uses one root; bound extra roots in test/support hosts.
                if failure.len() == 4 {
                    failure.remove(0);
                }
                failure.push((
                    root.to_owned(),
                    format!("{timestamp} [{category}] {}", &message[..end]),
                ));
            }
        }
    }
    let parts = message.chars().count().max(1).div_ceil(8192);
    let mut chars = message.chars();
    for index in 0..parts {
        let text: String = chars.by_ref().take(8192).collect();
        let suffix = if parts == 1 {
            String::new()
        } else {
            format!(" part={}/{}", index + 1, parts)
        };
        let line = format!("{timestamp} [{category}{suffix}] {text}\n");
        if append_line(&directory, &line).is_err() {
            return;
        }
    }
}

pub fn first_failure(root: &Path) -> Option<String> {
    FIRST_FAILURE.lock().ok().and_then(|value| {
        value
            .iter()
            .find(|(owner, _)| owner == root)
            .map(|(_, text)| text.clone())
    })
}

pub fn support_log_tail(root: &Path, maximum_bytes: usize) -> String {
    if maximum_bytes == 0 {
        return String::new();
    }
    // Rotation and tail assembly share a lock so segments cannot be duplicated
    // or skipped by a concurrent logger in this broker.
    let Ok(_guard) = LOG_LOCK.lock() else {
        return String::new();
    };
    let directory = root.join("logs");
    let mut newest_first = vec![directory.join("desktop.log")];
    newest_first
        .extend((1..=LOG_ARCHIVE_COUNT).map(|i| directory.join(format!("desktop.{i}.log"))));
    let mut remaining = maximum_bytes;
    let mut chunks = Vec::new();
    for path in newest_first {
        if remaining == 0 {
            break;
        }
        let Ok(mut file) = std::fs::File::open(path) else {
            continue;
        };
        let Ok(size) = file.metadata().map(|m| m.len()) else {
            continue;
        };
        let count = size.min(remaining as u64) as usize;
        if file.seek(SeekFrom::Start(size - count as u64)).is_err() {
            continue;
        }
        let mut chunk = vec![0; count];
        if file.read_exact(&mut chunk).is_err() {
            continue;
        }
        remaining -= count;
        chunks.push(chunk);
    }
    chunks.reverse();
    let bytes = chunks.concat();
    // A cut may leave TWO or THREE continuation bytes, not just one.
    let skip = bytes.iter().take_while(|b| (**b & 0xC0) == 0x80).count();
    String::from_utf8_lossy(&bytes[skip..]).into_owned()
}

/// Retain at most a partial UTF-8 scalar between bounded worker-pipe reads.
#[derive(Default)]
pub struct Utf8LogDecoder {
    tail: Vec<u8>,
}
impl Utf8LogDecoder {
    pub fn push(&mut self, bytes: &[u8]) -> String {
        self.tail.extend_from_slice(bytes);
        let mut output = String::new();
        let mut offset = 0;
        while offset < self.tail.len() {
            match std::str::from_utf8(&self.tail[offset..]) {
                Ok(text) => {
                    output.push_str(text);
                    offset = self.tail.len();
                }
                Err(error) => {
                    let valid = error.valid_up_to();
                    output
                        .push_str(std::str::from_utf8(&self.tail[offset..offset + valid]).unwrap());
                    offset += valid;
                    match error.error_len() {
                        Some(count) => {
                            output.push('\u{FFFD}');
                            offset += count;
                        }
                        None => break,
                    }
                }
            }
        }
        self.tail.drain(..offset);
        debug_assert!(self.tail.len() <= 3);
        output
    }
    pub fn finish(&mut self) -> String {
        let text = String::from_utf8_lossy(&self.tail).into_owned();
        self.tail.clear();
        text
    }
}
