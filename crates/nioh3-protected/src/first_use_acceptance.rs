//! Local, opt-in host acceptance for the ordinary first read. No CE or live writes.
//! Copy only the reads issued by the built-in PC202 product path into private
//! host evidence. This module is not compiled into the shipped worker.
use crate::{error::HostError, loadout, loadout_context, loadout_first_use, loadout_live};
use nioh3_loadout::{context::sha256, engine::Engine};
use nioh3_runtime::{
    combat_context::{read_combat_snapshot, SnapshotOutcome},
    InventoryMemory, InventoryProcess, RuntimeError,
};
use serde_json::{json, Value};
use std::{cell::RefCell, collections::BTreeMap, fs, path::Path};

struct Recorded<'a> {
    inner: &'a dyn InventoryMemory,
    reads: RefCell<Vec<(u64, Vec<u8>)>>,
}
impl InventoryMemory for Recorded<'_> {
    fn module_base(&self) -> u64 {
        self.inner.module_base()
    }
    fn process(&self) -> InventoryProcess {
        self.inner.process()
    }
    fn recheck_creation_filetime(&self) -> Result<u64, RuntimeError> {
        self.inner.recheck_creation_filetime()
    }
    fn read(&self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
        let bytes = self.inner.read(address, size)?;
        self.reads.borrow_mut().push((address, bytes.clone()));
        Ok(bytes)
    }
}
fn failed(e: impl std::fmt::Display) -> HostError {
    HostError::rejected(e.to_string())
}
fn json_file(path: &Path, x: &impl serde::Serialize) -> Result<(), HostError> {
    fs::write(path, serde_json::to_vec(x).map_err(failed)?).map_err(failed)
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

impl Recorded<'_> {
    fn save(&self, dir: &Path) -> Result<(), HostError> {
        let mut bytes = BTreeMap::new();
        let reads = self.reads.borrow();
        for (base, body) in reads.iter() {
            for (i, v) in body.iter().enumerate() {
                if bytes
                    .insert(base + i as u64, *v)
                    .is_some_and(|old| old != *v)
                {
                    // Preserve time order. A changing address has no single value
                    // that could faithfully represent both successful reads.
                    let mut stream = std::io::BufWriter::new(
                        fs::File::create(dir.join("ordered-reads.jsonl")).map_err(failed)?,
                    );
                    use std::io::Write;
                    for (address, value) in reads.iter() {
                        writeln!(stream, "{}", json!({"address":address,"bytes":hex(value)}))
                            .map_err(failed)?;
                    }
                    stream.flush().map_err(failed)?;
                    return Err(failed("host evidence changed; saved ordered reads only"));
                }
            }
        }
        let mut regions = Vec::new();
        let mut start = 0;
        let mut span = Vec::new();
        for (a, v) in bytes {
            if !span.is_empty() && a != start + span.len() as u64 {
                regions.push(json!({"address":start,"bytes":hex(&span)}));
                span.clear();
            }
            if span.is_empty() {
                start = a;
            }
            span.push(v);
        }
        if !span.is_empty() {
            regions.push(json!({"address":start,"bytes":hex(&span)}));
        }
        json_file(
            &dir.join("ordinary-read-memory.json"),
            &json!({"module_base":self.module_base(),"pid":self.process().pid,"creation_filetime":self.process().creation_filetime,"regions":regions}),
        )
    }
}

pub fn export(
    data: &Path,
    save_copy: &Path,
    out: &Path,
    fixture: Option<&Path>,
    id: &str,
) -> Result<(), HostError> {
    let bundle = loadout::find_bundle(data, &json!({"bundle":id}))?;
    let dir = bundle.directory(data);
    let saved = fs::read(save_copy).map_err(failed)?;
    let plain = nioh3_save::decrypt_container(&saved).map_err(HostError::from_save)?;
    fs::create_dir(out).map_err(failed)?;
    let capture = |memory: &dyn InventoryMemory| -> Result<(), HostError> {
        let memory = Recorded {
            inner: memory,
            reads: Default::default(),
        };
        let mut observed = None;
        let mut capture_role = || -> Result<(), HostError> {
            let SnapshotOutcome::Ready(snapshot) =
                read_combat_snapshot(&memory).map_err(HostError::from_runtime)?
            else {
                return Err(failed("current role is not stable"));
            };
            observed = Some(snapshot);
            let snap = observed
                .as_deref()
                .ok_or_else(|| failed("missing observed role"))?;
            json_file(&out.join("snapshot.json"), snap)?;
            if !loadout_context::outfit_is_owned(
                &snap.inputs.inventory,
                snap.inputs.active_bank as usize * 0xf70,
                &plain,
            )? {
                return Err(failed("equipped records differ from the save copy"));
            }
            let source = loadout_first_use::read(&memory, snap, &dir)?;
            json_file(&out.join("source-inputs.json"), &source.report())?;
            let context = source.context(&memory, snap, data, &dir)?;
            let engine = Engine::open_with_context(&dir, &context).map_err(failed)?;
            loadout_first_use::verify_equipped(&engine, snap)?;
            let bundle = out.join("bundle");
            let private = bundle.join("context");
            fs::create_dir_all(&private).map_err(failed)?;
            for entry in fs::read_dir(&dir).map_err(failed)? {
                let entry = entry.map_err(failed)?;
                if entry.file_type().map_err(failed)?.is_file() {
                    fs::copy(entry.path(), bundle.join(entry.file_name())).map_err(failed)?;
                }
            }
            json_file(&private.join("context.json"), context.manifest())?;
            fs::write(private.join("inventory.bin"), context.inventory()).map_err(failed)?;
            fs::write(private.join("frame.bin"), context.frame()).map_err(failed)?;
            let encoded = context.encode(&[]);
            for name in ["image.bin", "image_known.bin", "scene_entry.bin"] {
                fs::write(
                    private.join(name),
                    encoded
                        .file(name)
                        .ok_or_else(|| failed("missing private scene input"))?,
                )
                .map_err(failed)?;
            }
            let mut rows = Vec::new();
            for item in loadout::all_owned_search_items(data, &plain, id)? {
                let record = nioh3_save::character::equipment_record(&plain, item.slot_index)
                    .map_err(HostError::from_save)?;
                rows.push(json!({"slot":item.slot,"slot_index":item.slot_index,"record_sha256":sha256(&record[..0xe8]),"record_bytes":&record[..0xe8],"assignment_bytes":&record[0xe8..0xf0]}));
            }
            json_file(
                &out.join("inventory.json"),
                &json!({"schema":"nioh3-current-whole-backpack/v1","only_reads_save":true,"source_sha256":sha256(&saved),"searchable_items":rows.len(),"records":rows}),
            )?;
            Ok(())
        };
        let result = capture_role();
        if result.is_err() {
            if let Some(snapshot) = observed {
                let additional = loadout_first_use::sample_encounter(&memory, &snapshot, &dir);
                json_file(
                    &out.join("supplemental-encounter.json"),
                    &json!({
                        "scene":id, "sampled":additional.is_ok(), "admitted":false,
                        "error":additional.err().map(|e| e.to_string())
                    }),
                )?;
            }
        }
        let replay = memory.save(out);
        let info = json!({"ready":result.is_ok(),"replay_saved":replay.is_ok(),"scope":if fixture.is_some(){"retained memory"}else{"ordinary verified game read"},"save_sha256":sha256(&saved),"save_writes":0,"game_writes":0,"error":result.as_ref().err().map(ToString::to_string),"replay_error":replay.as_ref().err().map(ToString::to_string)});
        json_file(&out.join("receipt.json"), &info)?;
        result.and(replay)
    };
    if let Some(path) = fixture {
        let raw: Value =
            serde_json::from_slice(&fs::read(path).map_err(failed)?).map_err(failed)?;
        capture(&nioh3_runtime::FixtureMemory::from_json(&raw).map_err(HostError::from_runtime)?)
    } else {
        loadout_live::with_memory(|m| capture(m.ok_or_else(|| failed("game not running"))?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_reads_with_changed_bytes_preserve_both_observations(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = |bytes| {
            nioh3_runtime::FixtureMemory::from_json(&json!({
                "pid":1,"creation_filetime":2,"module_base":4096,
                "regions":[{"address":4100,"bytes":bytes}]
            }))
        };
        let before = fixture("01ff")?;
        let after = fixture("02ff")?;
        let mut record = Recorded {
            inner: &before,
            reads: Default::default(),
        };
        record.read(4100, 2)?;
        record.inner = &after;
        record.read(4100, 2)?;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("nioh-first-read-{nonce}"));
        fs::create_dir(&dir)?;
        assert!(record.save(&dir).is_err());
        assert!(!dir.join("ordinary-read-memory.json").exists());
        let log = dir.join("ordered-reads.jsonl");
        let rows: Vec<Value> = fs::read_to_string(&log)?
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?;
        assert_eq!(
            rows,
            [
                json!({"address":4100,"bytes":"01ff"}),
                json!({"address":4100,"bytes":"02ff"})
            ]
        );
        fs::remove_file(log)?;
        fs::remove_dir(dir)?;
        Ok(())
    }
}
