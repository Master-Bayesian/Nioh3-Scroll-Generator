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
use nioh3_protected::{serve, Contract, RoleApplication, RuntimeApplication, SaveApplication};
use nioh3_save::character::{
    equipment_counters, equipment_fields, equipment_offset, equipment_record, read_currency,
    Currency, EQUIPMENT_RECORD_BYTES, NEXT_GENERATION_SERIAL_OFFSET, NEXT_INVENTORY_KEY_OFFSET,
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
        if end > bytes.len() {
            break;
        }
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
    // The save-wide counters a new item takes its key and serial from.
    clear[NEXT_INVENTORY_KEY_OFFSET..NEXT_INVENTORY_KEY_OFFSET + 4]
        .copy_from_slice(&0xC86Fu32.to_le_bytes());
    clear[NEXT_GENERATION_SERIAL_OFFSET..NEXT_GENERATION_SERIAL_OFFSET + 4]
        .copy_from_slice(&0x26_B007u32.to_le_bytes());
    // The generator's player state as the owner's save serializes it:
    // difficulty 3, progress 6510 / 7710 on difficulties 1..3.
    let mut at = 0x73_D515;
    clear[at..at + 9].copy_from_slice(&[0x59, 0xD3, 0xC2, 0xA6, 1, 0, 0, 0, 3]);
    at += 9;
    for (tag, value) in [
        ([0xF8, 0xEE, 0x5E, 0xD3], 6510u32),
        ([0x21, 0xFD, 0xE5, 0xCE], 7710),
        ([0x80, 0xD3, 0x87, 0xB6], 0),
    ] {
        clear[at..at + 4].copy_from_slice(&tag);
        clear[at + 4..at + 8].copy_from_slice(&0x24u32.to_le_bytes());
        clear[at + 8..at + 12].copy_from_slice(&8u32.to_le_bytes());
        for index in 1..=3 {
            let slot = at + 12 + 4 * index;
            clear[slot..slot + 4].copy_from_slice(&value.to_le_bytes());
        }
        at += 44;
    }
    nioh3_save::patch_user_checksum(&mut clear).unwrap();

    let save_path = root
        .join(ACCOUNT_ID.to_string())
        .join("SAVEDATA00")
        .join("SAVEDATA.BIN");
    std::fs::create_dir_all(save_path.parent().unwrap()).unwrap();
    std::fs::write(&save_path, encrypt_container(&clear).unwrap()).unwrap();
    save_path
}

fn host_character_audit(exchange: &mut impl FnMut(&str, Value) -> Value, save_id: &str) -> Value {
    let character = job(exchange, "save.character", json!({"save_id": save_id}));
    assert_eq!(character["state"], "completed", "{character}");
    character["result"]["equipment"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["slot_index"] == SLOT)
        .unwrap()["audit"]
        .clone()
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
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        let snapshot = exchange("job.snapshot", json!({"job_id": job_id}));
        if snapshot["result"]["state"] != "running" {
            return snapshot["result"].clone();
        }
        assert!(
            Instant::now() < deadline,
            "character-edit job exceeded its bounded deadline"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

enum AcceptanceHostLifecycle {
    Running {
        sender: mpsc::Sender<Vec<u8>>,
        thread: thread::JoinHandle<()>,
    },
    Closed,
}

/// The existing real framed host, with retained request/response evidence.
struct AcceptanceHost {
    lifecycle: AcceptanceHostLifecycle,
    sink: Arc<Mutex<Vec<u8>>>,
    count: usize,
    transcript: Vec<Value>,
}

impl AcceptanceHost {
    fn new(application: Box<dyn RoleApplication>) -> Self {
        let contract = Contract::load(&repo_root().join("packages/contracts")).unwrap();
        let (sender, receiver) = mpsc::channel();
        let sink = Arc::new(Mutex::new(Vec::new()));
        let host_sink = Arc::clone(&sink);
        let host_thread = thread::spawn(move || {
            let _ = serve(
                application,
                &contract,
                &mut ChannelReader {
                    receiver,
                    buffer: Vec::new(),
                },
                &mut SharedSink(host_sink),
            );
        });
        Self {
            lifecycle: AcceptanceHostLifecycle::Running {
                sender,
                thread: host_thread,
            },
            sink,
            count: 0,
            transcript: Vec::new(),
        }
    }

    fn exchange(&mut self, method: &str, params: Value) -> Value {
        self.count += 1;
        let request =
            json!({"protocol":1,"id":self.count.to_string(),"method":method,"params":params});
        match &self.lifecycle {
            AcceptanceHostLifecycle::Running { sender, .. } => {
                sender.send(frame(&request)).unwrap()
            }
            AcceptanceHostLifecycle::Closed => panic!("acceptance host is closed"),
        }
        let response = wait_response(&self.sink, self.count)[self.count - 1].clone();
        self.transcript
            .push(json!({"request":request,"response":response}));
        response
    }

    fn job(&mut self, method: &str, params: Value) -> Value {
        job(
            &mut |method, params| self.exchange(method, params),
            method,
            params,
        )
    }

    fn close(&mut self) {
        assert_eq!(
            self.exchange("shutdown", json!({}))["result"]["safe_to_shutdown"],
            true
        );
        self.release();
    }

    fn release(&mut self) {
        match std::mem::replace(&mut self.lifecycle, AcceptanceHostLifecycle::Closed) {
            AcceptanceHostLifecycle::Running { sender, thread } => {
                drop(sender);
                thread.join().unwrap();
            }
            AcceptanceHostLifecycle::Closed => {}
        }
    }
}

impl Drop for AcceptanceHost {
    fn drop(&mut self) {
        self.release();
    }
}

fn effect_semantics(record: &[u8]) -> Vec<(u16, u32, i32, u8, u8, u8)> {
    (0..7)
        .filter_map(|index| {
            let entry = &record[0x34 + index * 0x18..0x4C + index * 0x18];
            let id = u32::from_le_bytes(entry[4..8].try_into().unwrap());
            (id != u32::MAX).then(|| {
                (
                    u16::from_le_bytes(entry[0..2].try_into().unwrap()),
                    id,
                    i32::from_le_bytes(entry[8..12].try_into().unwrap()),
                    entry[0x0C],
                    entry[0x0D] & 0x3F,
                    entry[0x0E] & 7,
                )
            })
        })
        .collect()
}

#[test]
fn hell_seed_search_review_commit_and_reload_preserve_the_native_candidate() {
    let evidence_base = std::env::var_os("NIOH3_HELL_E2E_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            build_root().join("deliverables/hell-weapon-seed-search-20261116/host-e2e")
        });
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = evidence_base.join(format!("run-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&evidence_base).unwrap();
    std::fs::create_dir(&root).unwrap();
    let save_path = fixture_save(&root);
    // The owner's long-running save reached this native u32 key range. A
    // synthetic low-key-only fixture would miss the old save-path u16 guard.
    let mut high_key_source = decrypt_container(&std::fs::read(&save_path).unwrap()).unwrap();
    high_key_source[NEXT_INVENTORY_KEY_OFFSET..NEXT_INVENTORY_KEY_OFFSET + 4]
        .copy_from_slice(&255_761u32.to_le_bytes());
    high_key_source[NEXT_GENERATION_SERIAL_OFFSET..NEXT_GENERATION_SERIAL_OFFSET + 4]
        .copy_from_slice(&11_349_840u32.to_le_bytes());
    nioh3_save::patch_user_checksum(&mut high_key_source).unwrap();
    std::fs::write(&save_path, encrypt_container(&high_key_source).unwrap()).unwrap();
    let original = std::fs::read(&save_path).unwrap();
    let data = repo_root().join("data");
    let engine = Engine::load(
        &data,
        &repo_root().join("packages/contracts"),
        None,
        ContextSelection::Production(GameFileVersion(2, 0, 2, 0)),
    )
    .unwrap();
    let mut runtime = AcceptanceHost::new(Box::new(
        RuntimeApplication::new(root.join("runtime-state"), &data, engine.context().clone())
            .unwrap(),
    ));
    let mut save = AcceptanceHost::new(Box::new(
        SaveApplication::new(root.join("save-state"), &data, engine.context().clone()).unwrap(),
    ));
    assert_eq!(runtime.exchange("handshake", json!({}))["ok"], true);
    assert_eq!(save.exchange("handshake", json!({}))["ok"], true);
    let registered = save.job("save.register", json!({"path":save_path}));
    let save_id = registered["result"]["save_id"]
        .as_str()
        .unwrap()
        .to_string();
    let character = save.job("save.character", json!({"save_id":save_id}));
    let source = character["result"]["source_sha256"].clone();
    let request = json!({"item_id":0x4BF7,"rarity":4,"level":170,"plus":22,"difficulty":1,
        "progress":[6510,7710,0,7710],"hell":true,"limit":200,
        "want":[0x8641,0x511C,0x9982,0xD7EF,0x590C]});
    let found = runtime.exchange("runtime.equipment_seeds", request.clone());
    assert_eq!(found["ok"], true, "{found}");
    let results = &found["result"];
    assert_eq!(results["generation_context"], "hell_conversion_zero_flags");
    let outcomes = results["outcomes"].as_array().unwrap();
    assert!(!outcomes.is_empty());
    for outcome in outcomes {
        for wanted in request["want"].as_array().unwrap() {
            assert!(outcome["effects"]
                .as_array()
                .unwrap()
                .iter()
                .any(|effect| effect["effect_id"] == *wanted));
        }
    }
    let selected = outcomes
        .iter()
        .find(|outcome| outcome["seed"] == 0x698C)
        .expect("known native conversion seed");
    let addition = json!({"item_id":results["item_id"],"rarity":results["rarity"],"level":results["level"],
        "plus":results["plus"],"difficulty":results["difficulty"],"hell":true,"hell_skill":0x3435,
        "seed":selected["seed"],"generation_context":results["generation_context"],
        "generation_progress":results["progress"],"generation_level":results["generation_level"],
        "candidate_digest":selected["candidate_digest"]});
    let mut checks = Vec::new();
    for (name, field, replacement) in [
        ("wrong-digest", "candidate_digest", json!("0".repeat(64))),
        ("wrong-progress", "generation_progress", json!([0, 0, 0, 0])),
        (
            "wrong-context",
            "generation_context",
            json!("reward_zero_flags"),
        ),
        ("changed-seed", "seed", json!(0x698D)),
        ("changed-level", "level", json!(171)),
        ("changed-generation-level", "generation_level", json!(171)),
        ("changed-plus", "plus", json!(23)),
        ("wrong-skill", "hell_skill", json!(0x8D12)),
        (
            "renderer-effects-override",
            "effects",
            json!([{"effect_id":0xFFFE,"value":1}]),
        ),
    ] {
        let mut changed = addition.clone();
        changed[field] = replacement;
        let rejected = save.job(
            "save.prepare_character_edit",
            json!({"save_id":save_id,"source_sha256":source,"add":[changed]}),
        );
        assert_eq!(rejected["state"], "failed", "{name}: {rejected}");
        assert_eq!(std::fs::read(&save_path).unwrap(), original);
        checks.push(json!({"name":name,"rejected_before_write":true,"response":rejected}));
    }
    let mut unbound = addition.clone();
    for field in [
        "candidate_digest",
        "generation_context",
        "generation_progress",
        "generation_level",
    ] {
        unbound.as_object_mut().unwrap().remove(field);
    }
    let rejected = save.job(
        "save.prepare_character_edit",
        json!({"save_id":save_id,"source_sha256":source,"add":[unbound]}),
    );
    assert_eq!(rejected["state"], "failed", "{rejected}");
    assert_eq!(std::fs::read(&save_path).unwrap(), original);
    checks.push(json!({"name":"unbound-hell","rejected_before_write":true,"response":rejected}));
    let mut impossible = request.clone();
    impossible["want"] = json!([0xFFFE]);
    let empty = runtime.exchange("runtime.equipment_seeds", impossible);
    assert_eq!(empty["ok"], true, "{empty}");
    assert_eq!(empty["result"]["matches"], 0);
    assert_eq!(empty["result"]["outcomes"], json!([]));
    let plan = save.job(
        "save.prepare_character_edit",
        json!({"save_id":save_id,"source_sha256":source,"add":[addition]}),
    );
    assert_eq!(plan["state"], "completed", "{plan}");
    let preview = &plan["result"]["preview"]["added"][0];
    assert_eq!(preview["after"]["hell"], true, "{preview}");
    assert_eq!(preview["after"]["hell_skill"], 0x3435, "{preview}");
    let slot = preview["slot_index"].as_u64().unwrap() as usize;
    let committed = save.job("save.commit", json!({"plan_id":plan["result"]["plan_id"]}));
    assert_eq!(
        committed["result"]["commit_status"], "committed",
        "{committed}"
    );
    let encrypted = std::fs::read(&save_path).unwrap();
    let clear = decrypt_container(&encrypted).unwrap();
    let record = equipment_record(&clear, slot).unwrap();
    let native_document: Value = serde_json::from_slice(
        &std::fs::read(
            repo_root().join("crates/nioh3-data/tests/fixtures/equipment_natural_v202.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let native_hex = native_document["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["source"] == "inserted-hell")
        .unwrap()["record"]
        .as_str()
        .unwrap();
    let native: Vec<u8> = (0..native_hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&native_hex[at..at + 2], 16).unwrap())
        .collect();
    assert_eq!(effect_semantics(record), effect_semantics(&native));
    assert_eq!(&record[0x10..0x12], &native[0x10..0x12]);
    assert_eq!(record[0x1A] & 0x10, 0x10);
    assert_eq!(equipment_counters(&clear).unwrap(), (255_762, 11_349_841));
    assert_eq!(equipment_fields(record).unwrap().inventory_key, 255_761);
    save.close();
    let mut reload_host = AcceptanceHost::new(Box::new(
        SaveApplication::new(root.join("save-state"), &data, engine.context().clone()).unwrap(),
    ));
    assert_eq!(reload_host.exchange("handshake", json!({}))["ok"], true);
    let registered_again = reload_host.job("save.register", json!({"path":save_path}));
    let reload_id = registered_again["result"]["save_id"].clone();
    let reloaded = reload_host.job("save.character", json!({"save_id":reload_id}));
    let row = reloaded["result"]["equipment"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["slot_index"] == slot)
        .unwrap();
    assert_eq!(row["hell"], true);
    assert_eq!(row["hell_skill"], 0x3435);
    runtime.close();
    reload_host.close();
    std::fs::write(
        root.join("reload-transcript.json"),
        serde_json::to_vec_pretty(&reload_host.transcript).unwrap(),
    )
    .unwrap();
    std::fs::write(
        root.join("runtime-transcript.json"),
        serde_json::to_vec_pretty(&runtime.transcript).unwrap(),
    )
    .unwrap();
    std::fs::write(
        root.join("save-transcript.json"),
        serde_json::to_vec_pretty(&save.transcript).unwrap(),
    )
    .unwrap();
    std::fs::write(root.join("stored-record.bin"), record).unwrap();
    let sources: Vec<Value> = ["crates/nioh3-domain/src/equipment_generation.rs",
        "crates/nioh3-protected/src/equipment_seeds.rs", "crates/nioh3-protected/tests/host_character_edit.rs",
        "crates/nioh3-save/src/character.rs",
        "packages/contracts/protected-request.schema.json", "packages/contracts/protected-response.schema.json",
        "crates/nioh3-data/tests/fixtures/equipment_natural_v202.json"].into_iter().map(|relative| {
            json!({"path":relative,"sha256":digest(&std::fs::read(repo_root().join(relative)).unwrap())})
        }).collect();
    std::fs::write(root.join("acceptance.json"),serde_json::to_vec_pretty(&json!({
        "pass":true,"scope":"real framed runtime search and encrypted owned-copy save transaction; no live game",
        "request":request,"selected":selected,"addition":addition,"plan":plan,"commit":committed,
        "reloaded":row,"rejections":checks,"source_sha256":digest(&original),
        "saved_sha256":digest(&encrypted),"record_sha256":digest(record),
        "native_fixture_semantics_equal":true,"game_or_real_save_accessed":false,
        "sources":sources,"generation_context_digest":engine.context().digest(),
        "repeat_command":["cargo","test","--locked","--offline","--manifest-path",
            "crates/nioh3-protected/Cargo.toml","--test","host_character_edit",
            "hell_seed_search_review_commit_and_reload_preserve_the_native_candidate","--","--nocapture"],
    })).unwrap()).unwrap();
    println!("Hell equipment E2E artifact: {}", root.display());
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
    let data = repo_root().join("data");
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

    // A live count or scroll edit follows its scroll into a save the game has
    // written since the inventory was read, rather than refusing every edit
    // after an autosave.
    let snapshot_id = inventory["result"]["snapshot_id"].clone();
    let source_of = |exchange: &mut dyn FnMut(&str, Value) -> Value| {
        let mut call = |method: &str, params: Value| exchange(method, params);
        job(
            &mut call,
            "save.count_edit_source",
            json!({"save_id": save_id, "snapshot_id": snapshot_id, "slot_index": 0}),
        )
    };
    let before_autosave = source_of(&mut exchange);
    assert_eq!(before_autosave["state"], "completed", "{before_autosave}");
    let mut autosaved = decrypt_container(&original).unwrap();
    let gold = 0x3D_DBBD + 16 + 8;
    autosaved[gold..gold + 8].copy_from_slice(&19_072_000u64.to_le_bytes());
    nioh3_save::patch_user_checksum(&mut autosaved).unwrap();
    let autosaved = encrypt_container(&autosaved).unwrap();
    std::fs::write(&save_path, &autosaved).unwrap();
    let after_autosave = source_of(&mut exchange);
    assert_eq!(after_autosave["state"], "completed", "{after_autosave}");
    let (old, new) = (
        &before_autosave["result"]["count_source"],
        &after_autosave["result"]["count_source"],
    );
    assert_eq!(old["record_hex"], new["record_hex"]);
    assert!(new["source_sha256"]
        .as_str()
        .unwrap()
        .eq_ignore_ascii_case(&digest(&autosaved)));
    std::fs::write(&save_path, &original).unwrap();

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

    // Real framed-host refusal: the game-like encrypted save changes after the
    // reviewed plan, before write intent. No phantom operation is created.
    let mut changed_plain = decrypt_container(&original).unwrap();
    let changed_slot = equipment_offset(SLOT).unwrap();
    changed_plain[changed_slot + 0x0A..changed_slot + 0x0C].copy_from_slice(&6u16.to_le_bytes());
    nioh3_save::patch_user_checksum(&mut changed_plain).unwrap();
    let changed_save = encrypt_container(&changed_plain).unwrap();
    std::fs::write(&save_path, &changed_save).unwrap();
    let refused = job(
        &mut exchange,
        "save.commit",
        json!({"plan_id":plan["plan_id"]}),
    );
    assert_eq!(refused["state"], "failed", "{refused}");
    assert_eq!(
        refused["error"]["message"],
        "Save changed after preparation; no write attempted"
    );
    assert_eq!(std::fs::read(&save_path).unwrap(), changed_save);
    let history = job(&mut exchange, "save.operations", json!({"save_id":save_id}));
    assert!(history["result"]["operations"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["operation_id"] != plan["plan_id"]));
    let evidence = build_root().join("deliverables/codex-v083-install-selection-20260930/backend");
    std::fs::create_dir_all(&evidence).unwrap();
    std::fs::write(evidence.join("stale-plan-host.json"),serde_json::to_vec_pretty(&json!({"pass":true,
        "boundary":"real framed host, isolated synthetic encrypted save; no game/real save","refusal":refused,"history":history,"changedSaveUntouched":true})).unwrap()).unwrap();
    std::fs::write(&save_path, &original).unwrap();

    // The core refuses a commit after the outer intent exists: the intent closes
    // as not committed instead of reading as a claimed write.
    let refused_plan = job(
        &mut exchange,
        "save.prepare_character_edit",
        json!({
            "save_id":save_id,"source_sha256":source,"currencies":{"gold":19_072_716,"amrita":0},
        }),
    );
    assert_eq!(refused_plan["state"], "completed", "{refused_plan}");
    let refused_id = refused_plan["result"]["plan_id"].as_str().unwrap();
    let foreign = root
        .join("state")
        .join("protected-internal")
        .join("v2-operations");
    std::fs::create_dir_all(&foreign).unwrap();
    let foreign = foreign.join(format!("{}.json", "9".repeat(32)));
    std::fs::write(
        &foreign,
        json!({"operation_id":"9".repeat(32),"kind":"edit","outcome":"pending",
               "installed_sha256":null,"backup_id":null,"message":null})
        .to_string(),
    )
    .unwrap();
    let refused = job(&mut exchange, "save.commit", json!({"plan_id":refused_id}));
    assert_eq!(refused["state"], "failed", "{refused}");
    std::fs::remove_file(&foreign).unwrap();
    let closed = job(
        &mut exchange,
        "save.operation",
        json!({"plan_id":refused_id}),
    );
    assert_eq!(
        closed["result"]["commit_status"], "not_committed",
        "{closed}"
    );
    assert_eq!(std::fs::read(&save_path).unwrap(), original);

    // Restoring bytes does not restore the prepared file fingerprint. A fresh
    // review is required even after the synthetic source has its old hash.
    let fresh = job(
        &mut exchange,
        "save.prepare_character_edit",
        json!({
            "save_id":save_id,"source_sha256":source,"currencies":{"gold":19_072_715,"amrita":0},
            "equipment":[{"slot_index":SLOT,"patch":{"plus":20,"effects":[{"index":0,"effect_id":0x8D2B,"value":4}]}}]
        }),
    );
    assert_eq!(fresh["state"], "completed", "{fresh}");
    let plan = &fresh["result"];

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
    assert!(unexpected.is_empty(), "removal changed {unexpected:x?}");

    // Adding equipment writes a new record into the first free slot after the
    // occupied tail and advances the key and serial counters, as a purchase does.
    let character = job(&mut exchange, "save.character", json!({"save_id": save_id}));
    let source = character["result"]["source_sha256"]
        .as_str()
        .unwrap()
        .to_string();
    let unknown = job(
        &mut exchange,
        "save.prepare_character_edit",
        json!({"save_id": save_id, "source_sha256": source,
               "add": [{"item_id": 1, "level": 175, "rarity": 3, "effects": []}]}),
    );
    assert_eq!(unknown["state"], "failed", "{unknown}");
    let wooden_sword = json!({"item_id": 0x27BF, "level": 175, "rarity": 3, "effects": [
        {"effect_id": 0x68F1, "value": 42},
        {"effect_id": 0x1589, "value": 80},
        {"effect_id": 0x16E5, "value": 11},
    ]});
    let addition = job(
        &mut exchange,
        "save.prepare_character_edit",
        json!({"save_id": save_id, "source_sha256": source, "add": [wooden_sword]}),
    );
    assert_eq!(addition["state"], "completed", "{addition}");
    let added = addition["result"]["preview"]["added"].as_array().unwrap();
    assert_eq!(added.len(), 1);
    let new_slot = added[0]["slot_index"].as_u64().unwrap() as usize;
    assert_eq!(added[0]["after"]["item_id"], 0x27BF);
    assert!(added[0]["audit"].is_object(), "{addition}");
    let before = std::fs::read(&save_path).unwrap();
    let receipt = job(
        &mut exchange,
        "save.commit",
        json!({"plan_id": addition["result"]["plan_id"].as_str().unwrap()}),
    );
    assert_eq!(receipt["result"]["commit_status"], "committed", "{receipt}");
    let before = decrypt_container(&before).unwrap();
    let after = decrypt_container(&std::fs::read(&save_path).unwrap()).unwrap();
    let record = equipment_record(&after, new_slot).unwrap();
    let fields = equipment_fields(record).unwrap();
    assert_eq!(
        (
            fields.item_id,
            fields.level,
            fields.rarity,
            fields.inventory_key
        ),
        (0x27BF, 175, 3, 0xC86F)
    );
    assert_eq!(
        fields
            .effects
            .iter()
            .filter(|(id, _)| *id != u32::MAX)
            .count(),
        3
    );
    assert!(!fields.worn);
    assert_eq!(equipment_counters(&after).unwrap(), (0xC870, 0x26_B008));
    let new_at = equipment_offset(new_slot).unwrap();
    let unexpected: Vec<usize> = (0..before.len())
        .filter(|index| before[*index] != after[*index])
        .filter(|offset| {
            !((new_at..new_at + EQUIPMENT_RECORD_BYTES).contains(offset)
                || (NEXT_INVENTORY_KEY_OFFSET..NEXT_INVENTORY_KEY_OFFSET + 4).contains(offset)
                || (NEXT_GENERATION_SERIAL_OFFSET..NEXT_GENERATION_SERIAL_OFFSET + 4)
                    .contains(offset)
                || (checksum..checksum + 4).contains(offset))
        })
        .collect();
    assert!(unexpected.is_empty(), "addition changed {unexpected:x?}");

    // A legal add names a seed: the plan writes the record the game's
    // generator builds from it, with the difficulty and progress re-read
    // from the save.
    let character = job(&mut exchange, "save.character", json!({"save_id": save_id}));
    let generation = &character["result"]["generation"];
    assert_eq!(generation["difficulty"], 3, "{character}");
    assert_eq!(
        generation["difficulties"][2],
        json!({"difficulty": 3, "progress": [6510, 7710, 0, 7710]})
    );
    let source = character["result"]["source_sha256"]
        .as_str()
        .unwrap()
        .to_string();
    let unplayed = job(
        &mut exchange,
        "save.prepare_character_edit",
        json!({"save_id": save_id, "source_sha256": source,
               "add": [{"item_id": 0x27BF, "level": 170, "rarity": 4, "seed": 0x3A5C, "difficulty": 5}]}),
    );
    assert_eq!(unplayed["state"], "failed", "{unplayed}");
    let seeded = json!({"item_id": 0x27BF, "level": 170, "plus": 2, "rarity": 4, "seed": 0x3A5C, "difficulty": 3});
    let addition = job(
        &mut exchange,
        "save.prepare_character_edit",
        json!({"save_id": save_id, "source_sha256": source, "add": [seeded]}),
    );
    assert_eq!(addition["state"], "completed", "{addition}");
    let added = &addition["result"]["preview"]["added"][0];
    assert_eq!(added["seeded"], true);
    assert_eq!(added["audit"]["natural"], true, "{addition}");
    let seeded_slot = added["slot_index"].as_u64().unwrap() as usize;
    let before = std::fs::read(&save_path).unwrap();
    let receipt = job(
        &mut exchange,
        "save.commit",
        json!({"plan_id": addition["result"]["plan_id"].as_str().unwrap()}),
    );
    assert_eq!(receipt["result"]["commit_status"], "committed", "{receipt}");
    let before = decrypt_container(&before).unwrap();
    let after = decrypt_container(&std::fs::read(&save_path).unwrap()).unwrap();
    let state = nioh3_domain::equipment_generation::PlayerState {
        type_class: 3,
        progress: [6510, 7710, 0, 7710],
    };
    let generated = nioh3_protected::equipment_seeds::generated_record(
        &repo_root().join("data"),
        &seeded,
        state,
    )
    .unwrap();
    let expected = nioh3_save::character::build_generated_equipment_record(
        equipment_record(&before, seeded_slot).unwrap(),
        &generated,
        0xC870,
        0x26_B008,
    )
    .unwrap();
    assert_eq!(
        equipment_record(&after, seeded_slot).unwrap(),
        expected.as_slice()
    );
    assert_eq!(equipment_counters(&after).unwrap(), (0xC871, 0x26_B009));

    let shutdown = exchange("shutdown", json!({}));
    assert_eq!(shutdown["result"]["safe_to_shutdown"], true, "{shutdown}");
    drop(sender);
    host.join().unwrap();
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn seeded_replay_audit_preserves_natural_and_replaced_group_key_verdicts() {
    let root = build_root()
        .join("tmp")
        .join(format!("nioh3-seed-group-audit-{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    let save_path = fixture_save(&root);
    let data = repo_root().join("data");
    let contracts = repo_root().join("packages").join("contracts");

    // Construct the same generated inventory record the production add path stores.
    let request = json!({
        "item_id": 0x27BF, "level": 170, "plus": 2, "rarity": 4,
        "seed": 0x3A5C, "difficulty": 3,
    });
    let state = nioh3_domain::equipment_generation::PlayerState {
        type_class: 3,
        progress: [6510, 7710, 0, 7710],
    };
    let generated =
        nioh3_protected::equipment_seeds::generated_record(&data, &request, state).unwrap();
    let natural_record = nioh3_save::character::build_generated_equipment_record(
        &vec![0; EQUIPMENT_RECORD_BYTES],
        &generated,
        0xC86F,
        0x26_B007,
    )
    .unwrap();
    let group_offset = (0..7)
        .map(|index| 0x34 + index * 0x18)
        .find(|offset| {
            u32::from_le_bytes(natural_record[offset + 4..offset + 8].try_into().unwrap())
                != u32::MAX
        })
        .expect("the seeded record has an occupied effect");
    let original_group = u16::from_le_bytes(
        natural_record[group_offset..group_offset + 2]
            .try_into()
            .unwrap(),
    );
    let mutated_group = original_group ^ 1;
    assert_ne!(original_group, mutated_group);
    let mut mutated_record = natural_record.clone();
    mutated_record[group_offset..group_offset + 2].copy_from_slice(&mutated_group.to_le_bytes());
    for (index, (before, after)) in natural_record.iter().zip(&mutated_record).enumerate() {
        if !(group_offset..group_offset + 2).contains(&index) {
            assert_eq!(before, after, "unexpected record mutation at {index:#x}");
        }
    }

    // Install the natural record in the synthetic save, retaining a valid save checksum.
    let encrypted = std::fs::read(&save_path).unwrap();
    let mut plain = decrypt_container(&encrypted).unwrap();
    let record_offset = equipment_offset(SLOT).unwrap();
    plain[record_offset..record_offset + EQUIPMENT_RECORD_BYTES].copy_from_slice(&natural_record);
    nioh3_save::patch_user_checksum(&mut plain).unwrap();
    std::fs::write(&save_path, encrypt_container(&plain).unwrap()).unwrap();

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

    let natural_audit = host_character_audit(&mut exchange, &save_id);

    // Change only one occupied entry's two-byte group key, then rewrite the
    // synthetic encrypted save through its normal checksum/encryption format.
    let mut plain = decrypt_container(&std::fs::read(&save_path).unwrap()).unwrap();
    plain[record_offset + group_offset..record_offset + group_offset + 2]
        .copy_from_slice(&mutated_group.to_le_bytes());
    nioh3_save::patch_user_checksum(&mut plain).unwrap();
    std::fs::write(&save_path, encrypt_container(&plain).unwrap()).unwrap();

    let mutated_audit = host_character_audit(&mut exchange, &save_id);

    let shutdown = exchange("shutdown", json!({}));
    assert_eq!(shutdown["result"]["safe_to_shutdown"], true, "{shutdown}");
    drop(sender);
    host.join().unwrap();
    std::fs::remove_dir_all(&root).ok();

    assert_eq!(natural_audit["natural"], true, "{natural_audit}");
    assert_eq!(
        mutated_audit["natural"], false,
        "changing only a generated entry's group key must invalidate replay: {mutated_audit}"
    );
    assert!(
        mutated_audit["findings"]
            .as_array()
            .is_some_and(|findings| findings
                .iter()
                .any(|finding| finding["code"] == "replaced_effect")),
        "the structural group mismatch finding must remain visible: {mutated_audit}"
    );
    assert_ne!(mutated_audit["replayed"], true, "{mutated_audit}");
}
