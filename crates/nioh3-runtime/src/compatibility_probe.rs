//! Bounded, read-only character-layout diagnostics for an unidentified build.
//!
//! A match is structural evidence only. This module has no write, allocation,
//! profile-selection or native-executor interface, and accepts no caller RVAs.

use crate::character::{character_layout, read_character_with_layout};
use crate::inventory::InventoryMemory;
use crate::platform::ModuleRange;
use serde_json::{json, Value};

const CANDIDATES: [&str; 3] = ["2.0.0.2", "2.0.1.0", "2.0.2.0"];
const NOTE: &str = "Character structure checks only; a match does not verify field semantics, native code, resources, or permission to write.";

/// Inspect only the three registered character layouts through a read-only
/// source. The two global pointer ranges must fit before either is read.
pub fn probe_character_layouts(memory: &dyn InventoryMemory, module: ModuleRange) -> Value {
    let process = memory.process();
    let mut candidates = Vec::new();
    for version in CANDIDATES {
        let outcome = (|| -> Result<(), String> {
            if module.base != memory.module_base() || module.base.checked_add(module.size).is_none()
            {
                return Err("The declared module range does not match the read handle".into());
            }
            let layout = character_layout(version)
                .ok_or_else(|| "The registered character layout is unavailable".to_string())?;
            for (name, rva) in [
                ("player global", layout.player_pointer_rva),
                ("inventory global", layout.inventory_pointer_rva),
            ] {
                if !module.contains_offset(rva, 8) {
                    return Err(format!("{name} RVA {rva:#x} is outside the loaded module"));
                }
            }
            let character =
                read_character_with_layout(memory, layout).map_err(|error| error.to_string())?;
            // Character reads intentionally preserve currencies/equipment when
            // item arrays fail. A diagnostic match must not hide that failure.
            character
                .items
                .map_err(|error| format!("Item-array validation failed: {error}"))?;
            Ok(())
        })();
        candidates.push(json!({
            "profile_version": version,
            "matched": outcome.is_ok(),
            "error": outcome.err(),
        }));
    }
    let matches = candidates
        .iter()
        .filter(|candidate| candidate["matched"] == true)
        .count();
    json!({
        "status": "structure_only",
        "outcome": match matches { 0 => "none", 1 => "unique", _ => "multiple" },
        "observed_pid": process.pid,
        "observed_creation_filetime": process.creation_filetime.to_string(),
        "candidates": candidates,
        "note": NOTE,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::character::{LiveItemContainer, EQUIPMENT_OFFSET, PLAYER_VTABLE_RVA};
    use crate::inventory::InventoryProcess;
    use crate::RuntimeError;
    use std::cell::Cell;
    use std::collections::BTreeMap;

    const BASE: u64 = 0x7FF7_0000_0000;
    const PLAYER: u64 = 0x2_0000_0000;
    const SLOT: u64 = 0x3_0000_0000;

    // This source exposes only reads; mutation and native calls are not part
    // of the interface used by the production diagnostic either.
    struct Memory {
        bytes: BTreeMap<u64, u8>,
        reads: Cell<usize>,
        changed_birth: bool,
    }

    impl Memory {
        fn new(versions: &[&str]) -> Self {
            let mut memory = Self {
                bytes: BTreeMap::new(),
                reads: Cell::new(0),
                changed_birth: false,
            };
            memory.put(BASE + 0x3C, &0x80u32.to_le_bytes());
            memory.put(BASE + 0xD0, &0x500_0000u32.to_le_bytes());
            memory.put(PLAYER, &(BASE + PLAYER_VTABLE_RVA).to_le_bytes());
            for slot in 0..3 {
                memory.put(
                    BASE + PLAYER_VTABLE_RVA + slot * 8,
                    &(BASE + 0x1000).to_le_bytes(),
                );
            }
            memory.put(SLOT, &(PLAYER + EQUIPMENT_OFFSET - 0x10).to_le_bytes());
            for version in versions {
                let layout = character_layout(version).expect("registered fixture layout");
                memory.put(BASE + layout.player_pointer_rva, &PLAYER.to_le_bytes());
                memory.put(BASE + layout.inventory_pointer_rva, &SLOT.to_le_bytes());
            }
            for container in LiveItemContainer::ALL {
                let start = PLAYER + container.offset();
                let preceding: u64 = match container {
                    LiveItemContainer::Held => 2500,
                    LiveItemContainer::Storage => 4000,
                };
                memory.put(start - 8, &preceding.to_le_bytes());
                memory.put(
                    start + (container.slots() * 0xE8) as u64,
                    &(container.slots() as u64).to_le_bytes(),
                );
            }
            memory
        }

        fn put(&mut self, address: u64, bytes: &[u8]) {
            for (offset, byte) in bytes.iter().enumerate() {
                self.bytes.insert(address + offset as u64, *byte);
            }
        }

        fn module() -> ModuleRange {
            ModuleRange {
                base: BASE,
                size: 0x500_0000,
            }
        }
    }

    impl InventoryMemory for Memory {
        fn module_base(&self) -> u64 {
            BASE
        }
        fn process(&self) -> InventoryProcess {
            InventoryProcess {
                pid: 77,
                creation_filetime: 88,
            }
        }
        fn recheck_creation_filetime(&self) -> Result<u64, RuntimeError> {
            Ok(if self.changed_birth { 89 } else { 88 })
        }
        fn read(&self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
            self.reads.set(self.reads.get() + 1);
            let end = address
                .checked_add(size as u64)
                .ok_or(RuntimeError::RegionNotReadable { address, size })?;
            let allowed = [
                (BASE, BASE + 0x500_0000),
                (PLAYER, PLAYER + 0x30_0000),
                (SLOT, SLOT + 8),
            ];
            if !allowed
                .iter()
                .any(|(start, limit)| address >= *start && end <= *limit)
            {
                return Err(RuntimeError::RegionNotReadable { address, size });
            }
            Ok((address..end)
                .map(|offset| self.bytes.get(&offset).copied().unwrap_or(0))
                .collect())
        }
    }

    #[test]
    fn a_unique_unknown_build_structure_remains_diagnostic_only() {
        let memory = Memory::new(&["2.0.2.0"]);
        let report = probe_character_layouts(&memory, Memory::module());
        assert_eq!(report["outcome"], "unique");
        assert_eq!(report["status"], "structure_only");
        assert_eq!(report["candidates"][2]["matched"], true);
        assert!(report.get("write_capability").is_none());
        assert!(report.get("executor").is_none());
        assert!(memory.reads.get() > 0);
    }

    #[test]
    fn multiple_and_unmatched_candidates_do_not_select_a_layout() {
        let memory = Memory::new(&CANDIDATES);
        assert_eq!(
            probe_character_layouts(&memory, Memory::module())["outcome"],
            "multiple"
        );
        let memory = Memory::new(&[]);
        let report = probe_character_layouts(&memory, Memory::module());
        assert_eq!(report["outcome"], "none");
        assert!(report["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .all(|candidate| candidate["error"].is_string()));
    }

    #[test]
    fn global_ranges_are_checked_before_any_read() {
        let memory = Memory::new(&CANDIDATES);
        let report = probe_character_layouts(
            &memory,
            ModuleRange {
                base: BASE,
                size: 0x1000,
            },
        );
        assert_eq!(report["outcome"], "none");
        assert_eq!(memory.reads.get(), 0);
        assert!(report["candidates"][0]["error"]
            .as_str()
            .unwrap()
            .contains("outside the loaded module"));
    }

    #[test]
    fn failed_item_capacities_and_changed_process_birth_are_not_matches() {
        let mut memory = Memory::new(&["2.0.2.0"]);
        memory.put(
            PLAYER + LiveItemContainer::Held.offset() - 8,
            &1u64.to_le_bytes(),
        );
        let report = probe_character_layouts(&memory, Memory::module());
        assert_eq!(report["outcome"], "none");
        assert!(report["candidates"][2]["error"]
            .as_str()
            .unwrap()
            .contains("Item-array validation failed"));
        let mut memory = Memory::new(&["2.0.2.0"]);
        memory.changed_birth = true;
        assert_eq!(
            probe_character_layouts(&memory, Memory::module())["outcome"],
            "none"
        );
    }
}
