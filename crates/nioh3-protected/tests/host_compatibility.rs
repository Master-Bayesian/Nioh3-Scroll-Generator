#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Real framed host/schema flow with an owned synthetic save and scripted image.
//! Failure controls precede implementation in V083_VERSION_COMPATIBILITY_20261001.md.
use nioh3_protected::compatibility::{CompatibilitySession, ExecutableIdentity};
use nioh3_protected::{serve, Contract, HostError, JobContext, Role, RoleApplication};
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    collections::VecDeque,
    io::{Cursor, Read, Write},
    path::PathBuf,
    rc::Rc,
};

struct Fixture {
    session: CompatibilitySession,
    image: ExecutableIdentity,
    root: PathBuf,
    sources: Vec<PathBuf>,
    inspections: u32,
    memory: nioh3_runtime::inventory::FixtureMemory,
}
impl RoleApplication for Fixture {
    fn role(&self) -> Role {
        Role::Runtime
    }
    fn context_payload(&self) -> Value {
        json!({"product_version": "0.8.8","game_profile":"pc-v2.00.02-v2.01","resources_digest":"r","algorithm_version":"a","policy_version":"p","context_digest":"c","seed_accelerator_abi":2,"seed_accelerator_build_id":"b"})
    }
    fn direct(&mut self, method: &str, params: &Value) -> Result<Value, HostError> {
        if method == "runtime.status" {
            return Ok(
                json!({"override_state":"stopped","hit_count":0,"pending_remote_calls":0,"safe_to_shutdown":true,"error":null}),
            );
        }
        if method == "runtime.character_snapshot" {
            let layout = nioh3_runtime::character::character_layout(&self.image.version).unwrap();
            let read = nioh3_runtime::character::read_character_with_layout(&self.memory, layout)
                .map_err(HostError::from_runtime)?;
            return Ok(
                json!({"source":"runtime","game_version":self.image.version,"process_id":read.pid,"currencies":{"amrita":read.currencies[0].1,"gold":read.currencies[1].1},"equipment_slots":2500,"equipment":[],"items":null}),
            );
        }
        if method != "runtime.compatibility" {
            return Err(HostError::invalid_request());
        }
        let report = match params["action"].as_str() {
            Some("inspect") => {
                self.inspections += 1;
                if self.inspections == 2 {
                    self.image.creation_filetime += 1;
                }
                self.session.report(&self.image)
            }
            Some("prepare") => self.session.prepare(&self.image, &self.root, &self.sources),
            Some("accept") => self.session.accept(
                &self.image,
                params["plan_id"].as_str().unwrap_or(""),
                params["confirmed"] == true,
                params["backup_confirmed"] == true,
            )?,
            Some("cancel") => {
                self.session.cancel();
                self.session.report(&self.image)
            }
            _ => return Err(HostError::invalid_request()),
        };
        Ok(json!({"compatibility":report}))
    }
    fn run(&mut self, _: &str, _: Value, _: &JobContext) -> Result<Value, HostError> {
        Err(HostError::invalid_request())
    }
    fn shutdown(&mut self) -> Result<Value, HostError> {
        Ok(json!({"safe_to_shutdown":true}))
    }
}
fn frame(v: &Value) -> Vec<u8> {
    let b = serde_json::to_vec(v).unwrap();
    let mut out = (b.len() as u32).to_le_bytes().to_vec();
    out.extend(b);
    out
}
fn request(id: u32, method: &str, params: Value) -> Value {
    json!({"protocol":1,"id":id.to_string(),"method":method,"params":params})
}
fn decode(mut bytes: &[u8]) -> Vec<Value> {
    let mut values = Vec::new();
    while bytes.len() >= 4 {
        let n = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        values.push(serde_json::from_slice(&bytes[4..4 + n]).unwrap());
        bytes = &bytes[4 + n..];
    }
    values
}

// The fixture feeds the actual plan ID from the preceding wire response into
// subsequent requests. Production schema and application validation stay intact.
struct Conversation {
    requests: VecDeque<Value>,
    current: Cursor<Vec<u8>>,
    replies: Rc<RefCell<Vec<u8>>>,
    sent: Vec<Value>,
}
impl Read for Conversation {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.current.position() == self.current.get_ref().len() as u64 {
            let Some(mut next) = self.requests.pop_front() else {
                return Ok(0);
            };
            if next["params"]["plan_id"] == "$prepared_plan" {
                let prior = decode(&self.replies.borrow());
                let id = prior
                    .iter()
                    .rev()
                    .find_map(|reply| reply["result"]["compatibility"]["plan"]["plan_id"].as_str());
                next["params"]["plan_id"] =
                    json!(id.map(str::to_owned).unwrap_or_else(|| "0".repeat(64)));
            }
            self.sent.push(next.clone());
            self.current = Cursor::new(frame(&next));
        }
        self.current.read(buffer)
    }
}
struct Replies(Rc<RefCell<Vec<u8>>>);
impl Write for Replies {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn character_memory(version: &str) -> nioh3_runtime::inventory::FixtureMemory {
    let base = 0x7ff700000000u64;
    let player = 0x200000000u64;
    let manager = 0x300000000u64;
    let (player_rva, manager_rva, vtable) = match version {
        "2.0.0.2" => (0x4749820, 0x4749500, base + 0x4010000),
        "2.0.1.0" => (0x474d800, 0x474d4e0, base + 0x4010000),
        _ => (0x4751850, 0x4751530, base + 0x402da20),
    };
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let regions = vec![
        json!({"address":base+player_rva,"bytes":hex(&player.to_le_bytes())}),
        json!({"address":base+manager_rva,"bytes":hex(&manager.to_le_bytes())}),
        json!({"address":manager,"bytes":hex(&(player+0x370e0-0x10).to_le_bytes())}),
        json!({"address":player,"bytes":hex(&vtable.to_le_bytes())}),
        json!({"address":player+0x10,"bytes":hex(&[0u8;0x40])}),
        json!({"address":player+0x370e0,"bytes":hex(&vec![0u8;2500*0xf0])}),
        json!({"address":base+0x3c,"bytes":hex(&0x80u32.to_le_bytes())}),
        json!({"address":base+0x80+0x50,"bytes":hex(&0x5800000u32.to_le_bytes())}),
        json!({"address":vtable,"bytes":hex(&[(base+0x1000).to_le_bytes(),(base+0x2000).to_le_bytes(),(base+0x3000).to_le_bytes()].concat())}),
    ];
    nioh3_runtime::inventory::FixtureMemory::from_json(
        &json!({"module_base":base,"pid":42,"creation_filetime":"77","regions":regions}),
    )
    .unwrap()
}
#[test]
fn compatibility_consent_and_backups_through_the_real_host() {
    let root = PathBuf::from(
        std::env::var("NIOH3_BUILD_ROOT").unwrap_or_else(|_| "D:/Nioh3_v080_deliverables".into()),
    )
    .join("tmp/compatibility-framed-host");
    std::fs::create_dir_all(&root).unwrap();
    let save = root.join("owned-synthetic-SAVEDATA.BIN");
    let original = vec![0x5a; 4096];
    std::fs::write(&save, &original).unwrap();
    let contract =
        Contract::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts"))
            .unwrap();
    let mut evidence = Vec::new();
    for version in ["2.0.0.2", "2.0.1.0", "2.0.2.0"] {
        for found in [false, true] {
            let app = Fixture {
                session: Default::default(),
                image: ExecutableIdentity {
                    pid: 42,
                    creation_filetime: 77,
                    path: "D:/owned-fixture/Nioh3.exe".into(),
                    version: version.into(),
                    sha256: "a".repeat(64),
                },
                root: root.join(format!("{version}-{found}")),
                sources: if found { vec![save.clone()] } else { vec![] },
                inspections: 0,
                memory: character_memory(version),
            };
            let requests = vec![
                request(1, "handshake", json!({})),
                request(2, "runtime.compatibility", json!({"action":"inspect"})),
                request(
                    3,
                    "runtime.compatibility",
                    json!({"action":"accept","confirmed":true,"backup_confirmed":true}),
                ),
                request(4, "runtime.compatibility", json!({"action":"prepare"})),
                request(
                    5,
                    "runtime.compatibility",
                    json!({"action":"accept","plan_id":"$prepared_plan","confirmed":true,"backup_confirmed":false}),
                ),
                request(
                    6,
                    "runtime.compatibility",
                    json!({"action":"accept","plan_id":"$prepared_plan","confirmed":true,"backup_confirmed":true}),
                ),
                request(7, "runtime.compatibility", json!({"action":"inspect"})),
                request(8, "runtime.character_snapshot", json!({})),
                request(9, "shutdown", json!({})),
            ];
            let output = Rc::new(RefCell::new(Vec::new()));
            let mut input = Conversation {
                requests: requests.into(),
                current: Cursor::new(Vec::new()),
                replies: output.clone(),
                sent: Vec::new(),
            };
            serve(
                Box::new(app),
                &contract,
                &mut input,
                &mut Replies(output.clone()),
            )
            .unwrap();
            let replies = decode(&output.borrow());
            let requests = input.sent;
            assert_eq!(replies.len(), 9, "{replies:?}");
            assert_eq!(
                replies[7]["result"]["game_version"], version,
                "version-specific character read"
            );
            assert_eq!(replies[1]["result"]["compatibility"]["warning"], true);
            assert_eq!(replies[2]["ok"], false, "accepting before a backup attempt");
            assert_eq!(
                replies[3]["result"]["compatibility"]["backup"]["verified"],
                found
            );
            assert_eq!(
                replies[4]["ok"], false,
                "missing explicit backup confirmation"
            );
            assert_eq!(
                replies[5]["ok"], found,
                "a missing verified backup cannot be confirmed away"
            );
            if found {
                assert_eq!(replies[5]["result"]["compatibility"]["accepted"], true);
            }
            assert_eq!(
                replies[6]["result"]["compatibility"]["accepted"], false,
                "process birth invalidates consent"
            );
            if found {
                let path = replies[3]["result"]["compatibility"]["backup"]["paths"][0]
                    .as_str()
                    .unwrap();
                assert_eq!(std::fs::read(path).unwrap(), original);
            }
            assert_eq!(
                std::fs::read(&save).unwrap(),
                original,
                "backup must never edit source"
            );
            evidence.push(json!({"version":version,"automaticSaveFound":found,"requests":requests,"replies":replies}));
        }
    }
    let out = root
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("deliverables/codex-v083-compatibility-20261001/backend");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("framed-compatibility.json"),serde_json::to_vec_pretty(&json!({"pass":true,"boundary":"real framed host/schema; synthetic image identity and owned synthetic save; no real game/save writes","cases":evidence})).unwrap()).unwrap();
}
