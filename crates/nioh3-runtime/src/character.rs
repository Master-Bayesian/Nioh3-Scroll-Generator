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

/// One item record (consumables, materials, books, key items).
pub const ITEM_RECORD_BYTES: usize = 0xE8;
/// Every record array is followed by a u64 that equals its capacity (observed
/// live: 2500 after the equipment, 1500 after held items, 4000 after the
/// storehouse equipment, 400 after stored items).
const ARRAY_COUNT_BYTES: u64 = 8;
/// Storehouse equipment between the held and the stored items (0xE8 records).
const STOREHOUSE_EQUIPMENT_SLOTS: usize = 4000;
/// Item flags: a u32 count at `+4`, or a record always counted as one.
const ITEM_COUNT_FLAGS: u32 = 0x20_0000 | 0x80_0000;

/// The held and stored item arrays inside the player object. Live order:
/// equipment, held items, storehouse equipment (4000), stored items; the save
/// omits the storehouse equipment between them, so the live storage offset
/// cannot be taken from the save layout (it pointed at storehouse equipment
/// and the capacity check refused it, 2026-09-27). Records are identical to
/// the save's (`nioh3_save::character::ItemContainer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveItemContainer {
    /// 持有.
    Held,
    /// 仓库.
    Storage,
}

impl LiveItemContainer {
    pub const ALL: [Self; 2] = [Self::Held, Self::Storage];

    pub const fn slots(self) -> usize {
        match self {
            Self::Held => 1500,
            Self::Storage => 400,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Held => "held",
            Self::Storage => "storage",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|container| container.label() == label)
    }

    /// First record, relative to the player object.
    pub const fn offset(self) -> u64 {
        let held = EQUIPMENT_OFFSET
            + (EQUIPMENT_SLOTS * EQUIPMENT_RECORD_BYTES) as u64
            + ARRAY_COUNT_BYTES;
        match self {
            Self::Held => held,
            Self::Storage => {
                let storehouse_equipment =
                    held + (Self::Held.slots() * ITEM_RECORD_BYTES) as u64 + ARRAY_COUNT_BYTES;
                storehouse_equipment
                    + (STOREHOUSE_EQUIPMENT_SLOTS * ITEM_RECORD_BYTES) as u64
                    + ARRAY_COUNT_BYTES
            }
        }
    }

    /// The u64 in front of the array and the value it must hold: the capacity
    /// of the array before it.
    const fn preceding_capacity(self) -> u64 {
        match self {
            Self::Held => EQUIPMENT_SLOTS as u64,
            Self::Storage => STOREHOUSE_EQUIPMENT_SLOTS as u64,
        }
    }

    const fn bytes(self) -> usize {
        self.slots() * ITEM_RECORD_BYTES
    }
}

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
    let slot = u64_at(&memory.read(base + GLOBAL_SLOT_RVA, 8)?)?;
    let root = if slot == 0 {
        0
    } else {
        u64_at(&memory.read(slot, 8)?)?
    };
    if root == 0 {
        return Err(layout("no character is loaded"));
    }
    let vtable = u64_at(&memory.read(player, 8)?)?;
    if vtable != base + PLAYER_VTABLE_RVA {
        return Err(layout(
            "the player object's vtable does not match this build",
        ));
    }
    if root + CONTAINER_BIAS != player + EQUIPMENT_OFFSET {
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
    /// The held and stored item arrays, or why they did not pass the layout
    /// check. Equipment and currencies stay usable either way.
    pub items: Result<[Vec<u8>; 2], String>,
}

impl CharacterRead {
    pub fn record(&self, slot_index: usize) -> Option<&[u8]> {
        let start = slot_index.checked_mul(EQUIPMENT_RECORD_BYTES)?;
        self.equipment.get(start..start + EQUIPMENT_RECORD_BYTES)
    }

    pub fn item(&self, container: LiveItemContainer, slot_index: usize) -> Option<&[u8]> {
        let arrays = self.items.as_ref().ok()?;
        let array = match container {
            LiveItemContainer::Held => &arrays[0],
            LiveItemContainer::Storage => &arrays[1],
        };
        let start = slot_index.checked_mul(ITEM_RECORD_BYTES)?;
        array.get(start..start + ITEM_RECORD_BYTES)
    }
}

/// Read one item array and check it looks like item records.
fn read_item_array(
    memory: &dyn InventoryMemory,
    player: u64,
    container: LiveItemContainer,
) -> Result<Vec<u8>, String> {
    let start = player + container.offset();
    let array = memory
        .read(start, container.bytes())
        .map_err(|error| error.to_string())?;
    let word = |address: u64| {
        memory
            .read(address, 8)
            .map_err(|error| error.to_string())
            .and_then(|bytes| u64_at(&bytes).map_err(|error| error.to_string()))
    };
    let before = word(start - ARRAY_COUNT_BYTES)?;
    let after = word(start + container.bytes() as u64)?;
    if before != container.preceding_capacity() || after != container.slots() as u64 {
        return Err(format!(
            "the {} item array is not framed by the expected capacities ({before}, {after})",
            container.label()
        ));
    }
    for record in array.as_chunks::<ITEM_RECORD_BYTES>().0 {
        let occupied = record[0] != 0 || record[1] != 0;
        let flags = u32::from_le_bytes([record[0x18], record[0x19], record[0x1A], record[0x1B]]);
        if occupied && flags & ITEM_COUNT_FLAGS == 0 {
            return Err(format!(
                "the {} item array holds a record without item count flags",
                container.label()
            ));
        }
    }
    Ok(array)
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
    let items = read_item_array(memory, player, LiveItemContainer::Held).and_then(|held| {
        read_item_array(memory, player, LiveItemContainer::Storage).map(|stored| [held, stored])
    });
    let process = memory.process();
    Ok(CharacterRead {
        pid: process.pid,
        creation_filetime: process.creation_filetime,
        player,
        currencies,
        equipment,
        items,
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

/// Module global holding the inventory ("持有物品") menu, which lives for the
/// whole session whether or not it is on screen.
pub const INVENTORY_MENU_RVA: u64 = 0x45C_91A0;
/// The inventory menu's vtable.
pub const INVENTORY_MENU_VTABLE_RVA: u64 = 0x401_1278;
/// The item-detail widget embedded in the inventory menu.
pub const DETAIL_WIDGET_OFFSET: u64 = 0x6850;
/// The widget's pointer to the item it displays, i.e. the item under the cursor.
pub const DETAIL_ITEM_OFFSET: u64 = 0x1B0;
/// Menu bytes observed as 0/1 while open and 1/0 while closed.
pub const MENU_CLOSED_FLAG_OFFSET: u64 = 0x1C;
pub const MENU_OPEN_FLAG_OFFSET: u64 = 0x60F8;
/// The widget refresh reads its item pointer here (`mov rax,[rcx+0x1B0]`);
/// checking the bytes pins the field offset to this build.
pub const DETAIL_READ_SITE_RVA: u64 = 0x22A_EB47;
pub const DETAIL_READ_SITE_BYTES: [u8; 7] = [0x48, 0x8B, 0x81, 0xB0, 0x01, 0x00, 0x00];

/// What the inventory menu's cursor is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSelection {
    /// The inventory menu is not on screen.
    Closed,
    /// An owned equipment record.
    Equipment { slot_index: usize, item_id: u16 },
    /// A held or stored item record.
    Item {
        container: LiveItemContainer,
        slot_index: usize,
        item_id: u16,
    },
    /// Something outside the known arrays.
    Other { item_id: u16 },
}

/// Read the item under the inventory menu's cursor without touching the game.
///
/// Evidence: `docs/knowledge/V082_LIVE_CHARACTER_EQUIPMENT_RESEARCH_20260926.md`
/// ("Following the in-game selection"). Any mismatch fails closed.
pub fn read_menu_selection(memory: &dyn InventoryMemory) -> Result<MenuSelection, RuntimeError> {
    let base = memory.module_base();
    if memory.read(base + DETAIL_READ_SITE_RVA, DETAIL_READ_SITE_BYTES.len())?
        != DETAIL_READ_SITE_BYTES
    {
        return Err(RuntimeError::SignatureMismatch {
            site: "inventory_detail_item_read".to_string(),
            rva: DETAIL_READ_SITE_RVA,
        });
    }
    let player = locate_player(memory)?;
    let menu = u64_at(&memory.read(base + INVENTORY_MENU_RVA, 8)?)?;
    if menu == 0 {
        return Ok(MenuSelection::Closed);
    }
    if u64_at(&memory.read(menu, 8)?)? != base + INVENTORY_MENU_VTABLE_RVA {
        return Err(layout(
            "the inventory menu's vtable does not match this build",
        ));
    }
    let closed = memory.read(menu + MENU_CLOSED_FLAG_OFFSET, 1)?[0];
    let open = memory.read(menu + MENU_OPEN_FLAG_OFFSET, 1)?[0];
    if closed != 0 || open != 1 {
        return Ok(MenuSelection::Closed);
    }
    let item = u64_at(&memory.read(menu + DETAIL_WIDGET_OFFSET + DETAIL_ITEM_OFFSET, 8)?)?;
    if item == 0 {
        return Ok(MenuSelection::Closed);
    }
    let item_id = u16::from_le_bytes(
        memory
            .read(item, 2)?
            .try_into()
            .map_err(|_| layout("short item read"))?,
    );
    let container = player + EQUIPMENT_OFFSET;
    let span = (EQUIPMENT_SLOTS * EQUIPMENT_RECORD_BYTES) as u64;
    if (container..container + span).contains(&item) {
        let offset = item - container;
        if !offset.is_multiple_of(EQUIPMENT_RECORD_BYTES as u64) {
            return Err(layout(
                "the selected item is not aligned to an equipment record",
            ));
        }
        return Ok(MenuSelection::Equipment {
            slot_index: (offset / EQUIPMENT_RECORD_BYTES as u64) as usize,
            item_id,
        });
    }
    for container in LiveItemContainer::ALL {
        let start = player + container.offset();
        if (start..start + container.bytes() as u64).contains(&item) {
            let offset = item - start;
            if !offset.is_multiple_of(ITEM_RECORD_BYTES as u64) {
                return Err(layout("the selected item is not aligned to an item record"));
            }
            return Ok(MenuSelection::Item {
                container,
                slot_index: (offset / ITEM_RECORD_BYTES as u64) as usize,
                item_id,
            });
        }
    }
    Ok(MenuSelection::Other { item_id })
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
    /// A count change of one item record; only bytes `+4..+8` may differ.
    Item {
        container: LiveItemContainer,
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
            Self::Item {
                container,
                slot_index,
                expected,
                replacement,
            } => {
                let only_count = expected.len() == ITEM_RECORD_BYTES
                    && replacement.len() == ITEM_RECORD_BYTES
                    && expected[..4] == replacement[..4]
                    && expected[8..] == replacement[8..];
                if *slot_index >= container.slots() || !only_count {
                    return Err(layout("an item edit may change only one record's count"));
                }
                let offset = container.offset() + (*slot_index * ITEM_RECORD_BYTES) as u64;
                Ok((player + offset, replacement.clone()))
            }
        }
    }

    fn expected_bytes(&self) -> Vec<u8> {
        match self {
            Self::Currency { expected, .. } => expected.to_le_bytes().to_vec(),
            Self::Equipment { expected, .. } | Self::Item { expected, .. } => expected.clone(),
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
            // The capacity words that frame each live array.
            for container in LiveItemContainer::ALL {
                let start = PLAYER + container.offset();
                fake.put(start - 8, &container.preceding_capacity().to_le_bytes());
                fake.put(
                    start + container.bytes() as u64,
                    &(container.slots() as u64).to_le_bytes(),
                );
            }
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

    const MENU: u64 = 0x4_0000_0000;

    fn with_open_menu(item: u64) -> Fake {
        let fake = Fake::new();
        fake.put(BASE + DETAIL_READ_SITE_RVA, &DETAIL_READ_SITE_BYTES);
        fake.put(BASE + INVENTORY_MENU_RVA, &MENU.to_le_bytes());
        fake.put(MENU, &(BASE + INVENTORY_MENU_VTABLE_RVA).to_le_bytes());
        fake.put(MENU + MENU_OPEN_FLAG_OFFSET, &[1]);
        fake.put(
            MENU + DETAIL_WIDGET_OFFSET + DETAIL_ITEM_OFFSET,
            &item.to_le_bytes(),
        );
        fake
    }

    #[test]
    fn the_menu_cursor_resolves_to_an_equipment_slot() {
        let record = PLAYER + EQUIPMENT_OFFSET + 5 * EQUIPMENT_RECORD_BYTES as u64;
        assert_eq!(
            read_menu_selection(&with_open_menu(record)).unwrap(),
            MenuSelection::Equipment {
                slot_index: 5,
                item_id: 0x8D5B
            }
        );
        let elsewhere = 0x5_0000_0000u64;
        let fake = with_open_menu(elsewhere);
        fake.put(elsewhere, &0x24AFu16.to_le_bytes());
        assert_eq!(
            read_menu_selection(&fake).unwrap(),
            MenuSelection::Other { item_id: 0x24AF }
        );
    }

    #[test]
    fn a_closed_menu_or_foreign_build_does_not_follow() {
        let record = PLAYER + EQUIPMENT_OFFSET;
        let fake = with_open_menu(record);
        fake.put(MENU + MENU_CLOSED_FLAG_OFFSET, &[1]);
        fake.put(MENU + MENU_OPEN_FLAG_OFFSET, &[0]);
        assert_eq!(read_menu_selection(&fake).unwrap(), MenuSelection::Closed);
        let fake = with_open_menu(record);
        fake.put(MENU, &0u64.to_le_bytes());
        assert!(read_menu_selection(&fake).is_err());
        let fake = with_open_menu(record);
        fake.put(BASE + DETAIL_READ_SITE_RVA, &[0x90]);
        assert!(read_menu_selection(&fake).is_err());
        let fake = with_open_menu(record + 8);
        assert!(read_menu_selection(&fake).is_err());
    }

    fn put_item(
        fake: &Fake,
        container: LiveItemContainer,
        slot: usize,
        id: u16,
        count: u32,
    ) -> u64 {
        let at = PLAYER + container.offset() + (slot * ITEM_RECORD_BYTES) as u64;
        fake.put(at, &id.to_le_bytes());
        fake.put(at + 4, &count.to_le_bytes());
        fake.put(at + 0x18, &0x20_0000u32.to_le_bytes());
        at
    }

    #[test]
    fn item_arrays_are_read_after_the_equipment() {
        let fake = Fake::new();
        put_item(&fake, LiveItemContainer::Held, 161, 0x24AF, 3);
        put_item(&fake, LiveItemContainer::Storage, 2, 0x05E7, 6734);
        let read = read_character(&fake).unwrap();
        assert_eq!(
            &read.item(LiveItemContainer::Held, 161).unwrap()[..2],
            &0x24AFu16.to_le_bytes()
        );
        assert_eq!(
            &read.item(LiveItemContainer::Storage, 2).unwrap()[4..8],
            &6734u32.to_le_bytes()
        );
        // Held starts right after the equipment array and its count, as the
        // hovered 火男面具 did live (container + 0x927C8).
        assert_eq!(LiveItemContainer::Held.offset(), EQUIPMENT_OFFSET + 0x927C8);
        // Stored items sit after the 4000-slot storehouse equipment, where the
        // owner's 36 stored stacks were found live (player + 0x201118).
        assert_eq!(LiveItemContainer::Storage.offset(), 0x20_1118);

        let shifted = Fake::new();
        let start = PLAYER + LiveItemContainer::Storage.offset();
        shifted.put(start - 8, &0u64.to_le_bytes());
        assert!(read_character(&shifted).unwrap().items.is_err());

        let broken = Fake::new();
        let at = PLAYER + LiveItemContainer::Storage.offset();
        broken.put(at, &0x1234u16.to_le_bytes());
        let read = read_character(&broken).unwrap();
        assert!(read.items.is_err());
        assert!(read.record(5).is_some());
    }

    #[test]
    fn the_menu_cursor_resolves_to_an_item_slot() {
        let fake = with_open_menu(0);
        let at = put_item(&fake, LiveItemContainer::Held, 161, 0x24AF, 3);
        fake.put(
            MENU + DETAIL_WIDGET_OFFSET + DETAIL_ITEM_OFFSET,
            &at.to_le_bytes(),
        );
        assert_eq!(
            read_menu_selection(&fake).unwrap(),
            MenuSelection::Item {
                container: LiveItemContainer::Held,
                slot_index: 161,
                item_id: 0x24AF
            }
        );
    }

    #[test]
    fn an_item_edit_writes_only_the_count() {
        let fake = Fake::new();
        let at = put_item(&fake, LiveItemContainer::Storage, 2, 0x05E7, 6734);
        let read = read_character(&fake).unwrap();
        let original = read.item(LiveItemContainer::Storage, 2).unwrap().to_vec();
        let mut replacement = original.clone();
        replacement[4..8].copy_from_slice(&9999u32.to_le_bytes());
        let mut writer = Writer {
            fake: &fake,
            writes: Vec::new(),
            creation: 77,
        };
        let edit = LiveEdit::Item {
            container: LiveItemContainer::Storage,
            slot_index: 2,
            expected: original.clone(),
            replacement: replacement.clone(),
        };
        assert_eq!(
            apply_live_edits(&fake, &mut writer, 42, &[edit]),
            LiveEditOutcome::Verified
        );
        assert_eq!(writer.writes, vec![(at + 4, vec![0x0F, 0x27])]);

        let mut sneaky = replacement;
        sneaky[0] = 0x99;
        let rejected = LiveEdit::Item {
            container: LiveItemContainer::Storage,
            slot_index: 2,
            expected: read_character(&fake)
                .unwrap()
                .item(LiveItemContainer::Storage, 2)
                .unwrap()
                .to_vec(),
            replacement: sneaky,
        };
        let mut writer = Writer {
            fake: &fake,
            writes: Vec::new(),
            creation: 77,
        };
        assert!(matches!(
            apply_live_edits(&fake, &mut writer, 42, &[rejected]),
            LiveEditOutcome::Rejected(_)
        ));
        assert!(writer.writes.is_empty());
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
