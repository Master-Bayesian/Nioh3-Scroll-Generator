#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Protected-host acceptance for `save.character` and
//! `save.prepare_character_edit` through the ordinary `save.commit`.
//!
//! Failure scenarios checked before the happy path is trusted:
//! - a plan naming a stale source digest is refused and writes nothing;
//! - an edit of an empty equipment slot is refused;
//! - only the requested currency value and equipment bytes change, the user
//!   checksum is refreshed, and the commit leaves a backup of the original.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use nioh3_domain::record::ScrollRecordBytes;
use nioh3_protected::{serve, Contract, RoleApplication, SaveApplication};
use nioh3_save::character::{
    equipment_fields, equipment_offset, read_currency, Currency, EQUIPMENT_RECORD_BYTES,
};
use nioh3_save::{
    decrypt_container, encrypt_container,
    layout::{SCROLL_GROUP_OFFSET, SCROLL_RECORD_BYTES, USER_SAVE_BYTES},
};
use nioh3_worker::{ContextSelection, Engine, GameFileVersion};

const ACCOUNT_ID: u64 = 76561198000000123;
const SLOT: usize = 1316;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn build_root() -> PathBuf {
    std::env::var_os("NIOH3_BUILD_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"D:\Nioh3_v080_deliverables"))
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn frame(value: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(value).expect("serialises");
    let mut bytes = (body.len() as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&body);
    bytes
}

fn response_frames(bytes: &[u8]) -> Vec<Value> {
    let mut offset = 0usize;
    let mut output = Vec::new();
    while offset + 4 <= bytes.len() {
        let size = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let end = offset + 4 + size;
        output.push(serde_json::from_slice(&bytes[offset + 4..end]).expect("valid response"));
        offset = end;
    }
    output
}

struct ChannelReader {
    receiver: mpsc::Receiver<Vec<u8>>,
    buffer: Vec<u8>,
}

impl Read for ChannelReader {
    fn read(&mut self, target: &mut [u8]) -> std::io::Result<usize> {
        if self.buffer.is_empty() {
            self.buffer = self
                .receiver
                .recv()
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "closed"))?;
        }
        let count = target.len().min(self.buffer.len());
        target[..count].copy_from_slice(&self.buffer[..count]);
        self.buffer.drain(..count);
        Ok(count)
    }
}

struct SharedSink(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for SharedSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("sink lock").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The forged 甲斐国江 captured live on PC v2.02.
fn forged_record() -> Vec<u8> {
    let hex = [
        "5b8d5b8d0100af00af00000000000040000000000000000082010000d7c70000",
        "0100c3dc0000000017802600000000000300000046548c3f66a100000f000000",
        "4092003f0000803f00000000b6a70000e302000002000000551900be00000000",
        "000000008a520000763500003a0000004e0300d10000000000000000a64f0000",
        "24d5000000000000004c0154000000000000000000000000ffffffff00000000",
        "00000054000000000000000000000000ffffffff00000000000000ba00000000",
        "0000000000000000ffffffff000000000000002d000000000000000000000000",
        "0000000000000000ffffffff03000000",
    ]
    .concat();
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}

fn fixture_save(root: &Path) -> PathBuf {
    let mut clear = vec![0u8; USER_SAVE_BYTES];
    clear[..6].copy_from_slice(b"RNNUSR");
    let mut template = ScrollRecordBytes::zeroed();
    template.write_u16(0x00, 0xE604).unwrap();
    template.write_u8(0x30, 3).unwrap();
    template.write_u8(0x31, 3).unwrap();
    let mut owned = *template.as_bytes();
    owned[0x02..0x04].copy_from_slice(&((ACCOUNT_ID >> 48) as u16).to_le_bytes());
    owned[0x04..0x06].copy_from_slice(&((ACCOUNT_ID >> 32) as u16).to_le_bytes());
    owned[0x14..0x18].copy_from_slice(&(ACCOUNT_ID as u32).to_le_bytes());
    clear[SCROLL_GROUP_OFFSET..SCROLL_GROUP_OFFSET + SCROLL_RECORD_BYTES].copy_from_slice(&owned);

    let mut at = 0x3D_DBBD;
    for (currency, value) in [(Currency::Amrita, 0u64), (Currency::Gold, 19_072_714)] {
        clear[at..at + 4].copy_from_slice(&currency.key().to_le_bytes());
        clear[at + 4..at + 8].copy_from_slice(&8u32.to_le_bytes());
        clear[at + 8..at + 16].copy_from_slice(&value.to_le_bytes());
        at += 16;
    }
    let slot = equipment_offset(SLOT).unwrap();
    clear[slot..slot + EQUIPMENT_RECORD_BYTES].copy_from_slice(&forged_record());
    nioh3_save::patch_user_checksum(&mut clear).unwrap();

    let save_path = root
        .join(ACCOUNT_ID.to_string())
        .join("SAVEDATA00")
        .join("SAVEDATA.BIN");
    std::fs::create_dir_all(save_path.parent().unwrap()).unwrap();
    std::fs::write(&save_path, encrypt_container(&clear).unwrap()).unwrap();
    save_path
}

fn wait_response(sink: &Arc<Mutex<Vec<u8>>>, count: usize) -> Vec<Value> {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let values = response_frames(&sink.lock().expect("sink lock"));
        if values.len() >= count {
            return values;
        }
        assert!(Instant::now() < deadline, "protected host response timeout");
        thread::sleep(Duration::from_millis(5));
    }
}

fn job(exchange: &mut impl FnMut(&str, Value) -> Value, method: &str, params: Value) -> Value {
    let started = exchange(method, params);
    assert_eq!(started["ok"], true, "{method}: {started}");
    let job_id = started["result"]["job_id"].as_str().unwrap().to_string();
    loop {
        let snapshot = exchange("job.snapshot", json!({"job_id": job_id}));
        if snapshot["result"]["state"] != "running" {
            return snapshot["result"].clone();
        }
    }
}

#[test]
fn character_edits_commit_through_the_save_transaction() {
    let root = build_root()
        .join("tmp")
        .join(format!("nioh3-character-edit-{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    let save_path = fixture_save(&root);
    let original = std::fs::read(&save_path).unwrap();
    let data = repo_root().join("nioh3_scroll_editor").join("data");
    let contracts = repo_root().join("packages").join("contracts");
    let engine = Engine::load(
        &data,
        &contracts,
        None,
        ContextSelection::Production(GameFileVersion(2, 0, 2, 0)),
    )
    .unwrap();
    let application: Box<dyn RoleApplication> = Box::new(
        SaveApplication::new(root.join("state"), &data, engine.context().clone()).unwrap(),
    );
    let contract = Contract::load(&contracts).unwrap();
    let (sender, receiver) = mpsc::channel();
    let sink = Arc::new(Mutex::new(Vec::new()));
    let host_sink = Arc::clone(&sink);
    let host = thread::spawn(move || {
        serve(
            application,
            &contract,
            &mut ChannelReader {
                receiver,
                buffer: Vec::new(),
            },
            &mut SharedSink(host_sink),
        )
        .unwrap();
    });
    let mut count = 0usize;
    let mut exchange = |method: &str, params: Value| -> Value {
        count += 1;
        let request =
            json!({"protocol": 1, "id": count.to_string(), "method": method, "params": params});
        sender.send(frame(&request)).unwrap();
        wait_response(&sink, count)[count - 1].clone()
    };
    assert_eq!(exchange("handshake", json!({}))["ok"], true);
    let registered = job(&mut exchange, "save.register", json!({"path": save_path}));
    let save_id = registered["result"]["save_id"]
        .as_str()
        .unwrap()
        .to_string();
    let inventory = job(&mut exchange, "save.inventory", json!({"save_id": save_id}));
    assert_eq!(inventory["state"], "completed", "{inventory}");

    let character = job(&mut exchange, "save.character", json!({"save_id": save_id}));
    assert_eq!(character["state"], "completed", "{character}");
    let character = &character["result"];
    assert_eq!(character["currencies"]["gold"], 19_072_714);
    assert_eq!(character["currencies"]["amrita"], 0);
    let rows = character["equipment"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["slot_index"], SLOT);
    assert_eq!(rows[0]["item_id"], 0x8D5B);
    assert_eq!(rows[0]["effects"][0]["effect_id"], 0xA166);
    let source = character["source_sha256"].as_str().unwrap().to_string();
    assert!(source.eq_ignore_ascii_case(&digest(&original)));

    let stale = job(
        &mut exchange,
        "save.prepare_character_edit",
        json!({"save_id": save_id, "source_sha256": "0".repeat(64), "currencies": {"gold": 1}}),
    );
    assert_eq!(stale["state"], "failed", "{stale}");
    let empty = job(
        &mut exchange,
        "save.prepare_character_edit",
        json!({"save_id": save_id, "source_sha256": source,
               "equipment": [{"slot_index": 0, "patch": {"plus": 5}}]}),
    );
    assert_eq!(empty["state"], "failed", "{empty}");
    assert_eq!(std::fs::read(&save_path).unwrap(), original);

    let plan = job(
        &mut exchange,
        "save.prepare_character_edit",
        json!({
            "save_id": save_id,
            "source_sha256": source,
            "currencies": {"gold": 19_072_715, "amrita": 0},
            "equipment": [{"slot_index": SLOT, "patch": {
                "plus": 20,
                "effects": [{"index": 0, "effect_id": 0x8D2B, "value": 4}],
            }}],
        }),
    );
    assert_eq!(plan["state"], "completed", "{plan}");
    let plan = &plan["result"];
    assert_eq!(plan["kind"], "edit");
    assert_eq!(plan["preview"]["modded"], true);
    assert_eq!(plan["preview"]["currencies"].as_array().unwrap().len(), 1);
    assert_eq!(std::fs::read(&save_path).unwrap(), original);

    let receipt = job(
        &mut exchange,
        "save.commit",
        json!({"plan_id": plan["plan_id"].as_str().unwrap()}),
    );
    assert_eq!(receipt["state"], "completed", "{receipt}");
    assert_eq!(receipt["result"]["commit_status"], "committed", "{receipt}");

    let after = decrypt_container(&std::fs::read(&save_path).unwrap()).unwrap();
    let before = decrypt_container(&original).unwrap();
    assert_eq!(read_currency(&after, Currency::Gold).unwrap(), 19_072_715);
    let slot = equipment_offset(SLOT).unwrap();
    let fields = equipment_fields(&after[slot..slot + EQUIPMENT_RECORD_BYTES]).unwrap();
    assert_eq!(fields.plus, 20);
    assert_eq!(fields.effects[0], (0x8D2B, 4));
    let checksum = nioh3_save::codec::USER_CHECKSUM_VALUE_OFFSET;
    let changed: Vec<usize> = (0..before.len())
        .filter(|index| before[*index] != after[*index])
        .collect();
    let gold_value = 0x3D_DBBD + 16 + 8;
    let unexpected: Vec<usize> = changed
        .iter()
        .copied()
        .filter(|offset| {
            !((gold_value..gold_value + 8).contains(offset)
                || (slot..slot + EQUIPMENT_RECORD_BYTES).contains(offset)
                || (checksum..checksum + 4).contains(offset))
        })
        .collect();
    assert!(
        unexpected.is_empty(),
        "unexpected changes at {unexpected:x?}"
    );
    let backups = job(&mut exchange, "save.backups", json!({"save_id": save_id}));
    assert_eq!(backups["state"], "completed", "{backups}");

    // A save-file removal also takes a worn item out of its sets: the forged
    // record is worn (set word +0xEC = 3), which the live path refuses.
    assert!(fields.worn);
    let character = job(&mut exchange, "save.character", json!({"save_id": save_id}));
    let source = character["result"]["source_sha256"]
        .as_str()
        .unwrap()
        .to_string();
    let both = job(
        &mut exchange,
        "save.prepare_character_edit",
        json!({"save_id": save_id, "source_sha256": source,
               "equipment": [{"slot_index": SLOT, "patch": {"plus": 5}}],
               "remove": [{"slot_index": SLOT}]}),
    );
    assert_eq!(both["state"], "failed", "{both}");
    let removal = job(
        &mut exchange,
        "save.prepare_character_edit",
        json!({"save_id": save_id, "source_sha256": source, "remove": [{"slot_index": SLOT}]}),
    );
    assert_eq!(removal["state"], "completed", "{removal}");
    let removed = removal["result"]["preview"]["removed"].as_array().unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0]["before"]["item_id"], 0x8D5B);
    assert_eq!(removal["result"]["preview"]["modded"], false);
    let edited = std::fs::read(&save_path).unwrap();
    let receipt = job(
        &mut exchange,
        "save.commit",
        json!({"plan_id": removal["result"]["plan_id"].as_str().unwrap()}),
    );
    assert_eq!(receipt["result"]["commit_status"], "committed", "{receipt}");
    let before = decrypt_container(&edited).unwrap();
    let after = decrypt_container(&std::fs::read(&save_path).unwrap()).unwrap();
    let freed = &after[slot..slot + EQUIPMENT_RECORD_BYTES];
    assert!(nioh3_save::character::equipment_slot_is_empty(freed));
    assert!(!nioh3_save::character::equipment_is_worn(freed));
    let unexpected: Vec<usize> = (0..before.len())
        .filter(|index| before[*index] != after[*index])
        .filter(|offset| {
            !((slot..slot + EQUIPMENT_RECORD_BYTES).contains(offset)
                || (checksum..checksum + 4).contains(offset))
        })
        .collect();
    assert!(
        unexpected.is_empty(),
        "removal changed {unexpected:x?}"
    );

    let shutdown = exchange("shutdown", json!({}));
    assert_eq!(shutdown["result"]["safe_to_shutdown"], true, "{shutdown}");
    drop(sender);
    host.join().unwrap();
    std::fs::remove_dir_all(&root).ok();
}
