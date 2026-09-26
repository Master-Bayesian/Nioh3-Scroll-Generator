//! Live character currencies and owned equipment for PC v2.0.2.0.
//!
//! The player data object is reached through one module global and verified
//! by its vtable; owned equipment sits inside it at a fixed displacement, and
//! that address must also equal the container Codex's read-only inventory path
//! resolves, so the two independent roots cross-check each other. Evidence:
//! `docs/knowledge/V082_LIVE_CHARACTER_EQUIPMENT_RESEARCH_20260926.md`.
//!
//! Reads are taken twice and must agree. A write is a compare-and-swap: every
//! target is re-read and must still equal the value the caller reviewed, only
//! the changed bytes are written, and each target is read back. Nothing is
//! written when any check fails before the first write.

use crate::error::RuntimeError;
use crate::inventory::{InventoryMemory, CONTAINER_BIAS, GLOBAL_SLOT_RVA};
use crate::mutation::memory::TargetProcess;

/// The only build this layout was observed on.
pub const CHARACTER_GAME_VERSION: &str = "2.0.2.0";
/// Module global holding the player data object.
pub const PLAYER_POINTER_RVA: u64 = 0x475_1850;
/// The player data object's vtable.
pub const PLAYER_VTABLE_RVA: u64 = 0x402_DA20;
/// Owned equipment inside the player data object.
pub const EQUIPMENT_OFFSET: u64 = 0x3_70E0;
/// Slots in the owned-equipment array.
pub const EQUIPMENT_SLOTS: usize = 2500;
/// One equipment record.
pub const EQUIPMENT_RECORD_BYTES: usize = 0xF0;

/// A currency the player object stores as a 64-bit value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveCurrency {
    Amrita,
    Gold,
}

impl LiveCurrency {
    pub const ALL: [Self; 2] = [Self::Amrita, Self::Gold];

    pub const fn offset(self) -> u64 {
        match self {
            Self::Amrita => 0x10,
            Self::Gold => 0x18,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Amrita => "amrita",
            Self::Gold => "gold",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|currency| currency.label() == label)
    }
}

fn layout(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InventoryChain {
        detail: format!("character layout: {}", message.into()),
    }
}

fn u64_at(bytes: &[u8]) -> Result<u64, RuntimeError> {
    let window: [u8; 8] = bytes
        .get(..8)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| layout("short pointer read"))?;
    Ok(u64::from_le_bytes(window))
}

/// Resolve and verify the player data object.
pub fn locate_player(memory: &dyn InventoryMemory) -> Result<u64, RuntimeError> {
    let base = memory.module_base();
    let global = base
        .checked_add(PLAYER_POINTER_RVA)
        .ok_or_else(|| layout("player global overflowed"))?;
    let player = u64_at(&memory.read(global, 8)?)?;
    if player == 0 {
        return Err(layout("no character is loaded"));
    }
    let vtable = u64_at(&memory.read(player, 8)?)?;
    if vtable != base + PLAYER_VTABLE_RVA {
        return Err(layout(
            "the player object's vtable does not match this build",
        ));
    }
    let slot = u64_at(&memory.read(base + GLOBAL_SLOT_RVA, 8)?)?;
    let root = if slot == 0 {
        0
    } else {
        u64_at(&memory.read(slot, 8)?)?
    };
    if root == 0 || root + CONTAINER_BIAS != player + EQUIPMENT_OFFSET {
        return Err(layout(
            "the equipment container does not sit inside the player object",
        ));
    }
    Ok(player)
}

/// One consistent read of the character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterRead {
    pub pid: u32,
    pub creation_filetime: u64,
    pub player: u64,
    pub currencies: Vec<(LiveCurrency, u64)>,
    /// All `EQUIPMENT_SLOTS` records, occupied or not.
    pub equipment: Vec<u8>,
}

impl CharacterRead {
    pub fn record(&self, slot_index: usize) -> Option<&[u8]> {
        let start = slot_index.checked_mul(EQUIPMENT_RECORD_BYTES)?;
        self.equipment.get(start..start + EQUIPMENT_RECORD_BYTES)
    }
}

fn read_once(memory: &dyn InventoryMemory, player: u64) -> Result<CharacterRead, RuntimeError> {
    let mut currencies = Vec::new();
    for currency in LiveCurrency::ALL {
        let value = u64_at(&memory.read(player + currency.offset(), 8)?)?;
        currencies.push((currency, value));
    }
    let equipment = memory.read(
        player + EQUIPMENT_OFFSET,
        EQUIPMENT_SLOTS * EQUIPMENT_RECORD_BYTES,
    )?;
    let process = memory.process();
    Ok(CharacterRead {
        pid: process.pid,
        creation_filetime: process.creation_filetime,
        player,
        currencies,
        equipment,
    })
}

/// Read the character twice; publish only when both reads agree.
pub fn read_character(memory: &dyn InventoryMemory) -> Result<CharacterRead, RuntimeError> {
    let player = locate_player(memory)?;
    let first = read_once(memory, player)?;
    if locate_player(memory)? != player {
        return Err(layout("the player object moved during the read"));
    }
    let second = read_once(memory, player)?;
    if first != second {
        return Err(layout(
            "the character changed during the read; try again when the game is idle",
        ));
    }
    if memory.recheck_creation_filetime()? != first.creation_filetime {
        return Err(RuntimeError::ProcessInstanceChanged { pid: first.pid });
    }
    Ok(first)
}

/// One reviewed change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveEdit {
    Currency {
        currency: LiveCurrency,
        expected: u64,
        replacement: u64,
    },
    Equipment {
        slot_index: usize,
        expected: Vec<u8>,
        replacement: Vec<u8>,
    },
}

impl LiveEdit {
    fn target(&self, player: u64) -> Result<(u64, Vec<u8>), RuntimeError> {
        match self {
            Self::Currency {
                currency,
                replacement,
                ..
            } => Ok((
                player + currency.offset(),
                replacement.to_le_bytes().to_vec(),
            )),
            Self::Equipment {
                slot_index,
                expected,
                replacement,
            } => {
                if *slot_index >= EQUIPMENT_SLOTS
                    || expected.len() != EQUIPMENT_RECORD_BYTES
                    || replacement.len() != EQUIPMENT_RECORD_BYTES
                {
                    return Err(layout("an equipment edit is out of range"));
                }
                let offset = EQUIPMENT_OFFSET + (*slot_index * EQUIPMENT_RECORD_BYTES) as u64;
                Ok((player + offset, replacement.clone()))
            }
        }
    }

    fn expected_bytes(&self) -> Vec<u8> {
        match self {
            Self::Currency { expected, .. } => expected.to_le_bytes().to_vec(),
            Self::Equipment { expected, .. } => expected.clone(),
        }
    }
}

/// How one live edit ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveEditOutcome {
    /// Every target read back as its replacement.
    Verified,
    /// A check failed before the first write; nothing was written.
    Rejected(String),
    /// A write was attempted and its result is not proven.
    Uncertain(String),
}

/// Apply `edits` as compare-and-swap writes.
///
/// `memory` is the verified read view and `writer` a minimal write handle on
/// the same process instance.
pub fn apply_live_edits(
    memory: &dyn InventoryMemory,
    writer: &mut dyn TargetProcess,
    expected_pid: u32,
    edits: &[LiveEdit],
) -> LiveEditOutcome {
    let declared = memory.process();
    if declared.pid != expected_pid || writer.pid() != expected_pid {
        return LiveEditOutcome::Rejected(
            "the game process changed since the character was read".to_string(),
        );
    }
    match writer.creation_filetime() {
        Ok(Some(creation)) if creation == declared.creation_filetime => {}
        _ => {
            return LiveEditOutcome::Rejected(
                "the game process changed since the character was read".to_string(),
            )
        }
    }
    let player = match locate_player(memory) {
        Ok(player) => player,
        Err(error) => return LiveEditOutcome::Rejected(error.to_string()),
    };
    let mut planned = Vec::new();
    for edit in edits {
        let (address, replacement) = match edit.target(player) {
            Ok(target) => target,
            Err(error) => return LiveEditOutcome::Rejected(error.to_string()),
        };
        let expected = edit.expected_bytes();
        match memory.read(address, expected.len()) {
            Ok(current) if current == expected => {}
            Ok(_) => {
                return LiveEditOutcome::Rejected(
                    "the game changed a value since it was read; reload and try again".to_string(),
                )
            }
            Err(error) => return LiveEditOutcome::Rejected(error.to_string()),
        }
        planned.push((address, expected, replacement));
    }
    for (address, expected, replacement) in &planned {
        let first = expected.iter().zip(replacement).position(|(a, b)| a != b);
        let last = expected.iter().zip(replacement).rposition(|(a, b)| a != b);
        let (Some(first), Some(last)) = (first, last) else {
            continue;
        };
        if let Err(error) = writer.write(address + first as u64, &replacement[first..=last]) {
            return LiveEditOutcome::Uncertain(error.to_string());
        }
        match memory.read(*address, replacement.len()) {
            Ok(readback) if readback == *replacement => {}
            Ok(_) => {
                return LiveEditOutcome::Uncertain(
                    "a written value did not read back as written".to_string(),
                )
            }
            Err(error) => return LiveEditOutcome::Uncertain(error.to_string()),
        }
    }
    LiveEditOutcome::Verified
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::inventory::InventoryProcess;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    const BASE: u64 = 0x7FF7_0000_0000;
    const PLAYER: u64 = 0x2_0000_0000;

    struct Fake {
        bytes: RefCell<BTreeMap<u64, u8>>,
        creation: u64,
    }

    impl Fake {
        fn new() -> Self {
            let fake = Self {
                bytes: RefCell::new(BTreeMap::new()),
                creation: 77,
            };
            fake.put(BASE + PLAYER_POINTER_RVA, &PLAYER.to_le_bytes());
            fake.put(PLAYER, &(BASE + PLAYER_VTABLE_RVA).to_le_bytes());
            let slot = 0x3_0000_0000u64;
            fake.put(BASE + GLOBAL_SLOT_RVA, &slot.to_le_bytes());
            fake.put(
                slot,
                &(PLAYER + EQUIPMENT_OFFSET - CONTAINER_BIAS).to_le_bytes(),
            );
            fake.put(PLAYER + 0x18, &19_072_714u64.to_le_bytes());
            let record = PLAYER + EQUIPMENT_OFFSET + 5 * EQUIPMENT_RECORD_BYTES as u64;
            fake.put(record, &0x8D5Bu16.to_le_bytes());
            fake
        }

        fn put(&self, address: u64, data: &[u8]) {
            let mut bytes = self.bytes.borrow_mut();
            for (index, byte) in data.iter().enumerate() {
                bytes.insert(address + index as u64, *byte);
            }
        }
    }

    impl InventoryMemory for Fake {
        fn module_base(&self) -> u64 {
            BASE
        }
        fn process(&self) -> InventoryProcess {
            InventoryProcess {
                pid: 42,
                creation_filetime: self.creation,
            }
        }
        fn recheck_creation_filetime(&self) -> Result<u64, RuntimeError> {
            Ok(self.creation)
        }
        fn read(&self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
            let bytes = self.bytes.borrow();
            Ok((0..size as u64)
                .map(|offset| *bytes.get(&(address + offset)).unwrap_or(&0))
                .collect())
        }
    }

    struct Writer<'a> {
        fake: &'a Fake,
        writes: Vec<(u64, Vec<u8>)>,
        creation: u64,
    }

    impl TargetProcess for Writer<'_> {
        fn pid(&self) -> u32 {
            42
        }
        fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
            self.fake.read(address, size)
        }
        fn write(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError> {
            self.writes.push((address, data.to_vec()));
            self.fake.put(address, data);
            Ok(())
        }
        fn write_code(&mut self, _: u64, _: &[u8]) -> Result<(), RuntimeError> {
            Err(layout("not used"))
        }
        fn allocate_executable_near(&mut self, _: u64, _: usize) -> Result<u64, RuntimeError> {
            Err(layout("not used"))
        }
        fn free_allocation(&mut self, _: u64) -> Result<(), RuntimeError> {
            Ok(())
        }
        fn exited(&mut self) -> Result<bool, RuntimeError> {
            Ok(false)
        }
        fn creation_filetime(&mut self) -> Result<Option<u64>, RuntimeError> {
            Ok(Some(self.creation))
        }
        fn close(&mut self) {}
    }

    #[test]
    fn a_consistent_read_names_currencies_and_records() {
        let fake = Fake::new();
        let read = read_character(&fake).unwrap();
        assert_eq!(read.player, PLAYER);
        assert_eq!(read.currencies[1], (LiveCurrency::Gold, 19_072_714));
        assert_eq!(&read.record(5).unwrap()[..2], &0x8D5Bu16.to_le_bytes());
    }

    #[test]
    fn a_foreign_vtable_or_container_fails_closed() {
        let fake = Fake::new();
        fake.put(PLAYER, &0u64.to_le_bytes());
        assert!(read_character(&fake).is_err());
        let fake = Fake::new();
        fake.put(0x3_0000_0000, &0x1234u64.to_le_bytes());
        assert!(read_character(&fake).is_err());
    }

    #[test]
    fn edits_write_only_changed_bytes_after_every_check_passes() {
        let fake = Fake::new();
        let read = read_character(&fake).unwrap();
        let original = read.record(5).unwrap().to_vec();
        let mut replacement = original.clone();
        replacement[0x0A] = 20;
        let edits = [
            LiveEdit::Currency {
                currency: LiveCurrency::Gold,
                expected: 19_072_714,
                replacement: 19_072_715,
            },
            LiveEdit::Equipment {
                slot_index: 5,
                expected: original,
                replacement: replacement.clone(),
            },
        ];
        let mut writer = Writer {
            fake: &fake,
            writes: Vec::new(),
            creation: 77,
        };
        assert_eq!(
            apply_live_edits(&fake, &mut writer, 42, &edits),
            LiveEditOutcome::Verified
        );
        assert_eq!(writer.writes.len(), 2);
        assert_eq!(writer.writes[0], (PLAYER + 0x18, vec![0xCB]));
        let record = PLAYER + EQUIPMENT_OFFSET + 5 * EQUIPMENT_RECORD_BYTES as u64;
        assert_eq!(writer.writes[1], (record + 0x0A, vec![20]));
        let after = read_character(&fake).unwrap();
        assert_eq!(after.record(5).unwrap(), replacement.as_slice());
    }

    #[test]
    fn a_stale_value_or_another_process_writes_nothing() {
        let fake = Fake::new();
        let stale = [
            LiveEdit::Currency {
                currency: LiveCurrency::Amrita,
                expected: 1,
                replacement: 5,
            },
            LiveEdit::Currency {
                currency: LiveCurrency::Gold,
                expected: 19_072_714,
                replacement: 0,
            },
        ];
        let mut writer = Writer {
            fake: &fake,
            writes: Vec::new(),
            creation: 77,
        };
        assert!(matches!(
            apply_live_edits(&fake, &mut writer, 42, &stale),
            LiveEditOutcome::Rejected(_)
        ));
        assert!(writer.writes.is_empty());
        let fresh = [stale[1].clone()];
        assert!(matches!(
            apply_live_edits(&fake, &mut writer, 43, &fresh),
            LiveEditOutcome::Rejected(_)
        ));
        let mut recycled = Writer {
            fake: &fake,
            writes: Vec::new(),
            creation: 78,
        };
        assert!(matches!(
            apply_live_edits(&fake, &mut recycled, 42, &fresh),
            LiveEditOutcome::Rejected(_)
        ));
        assert!(writer.writes.is_empty() && recycled.writes.is_empty());
    }
}
