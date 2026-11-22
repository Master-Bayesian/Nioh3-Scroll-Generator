//! Selected prepared spell slots from the same PC202 actor as the optimizer.
use nioh3_loadout::peak_prepared::{summon_water_scale, PreparedWaterDonor};
use nioh3_runtime::{combat_context::CombatSnapshot, InventoryMemory};

pub(crate) const SUMMON_WATER: u16 = 0xCBC3;
pub(crate) const CARNAGE: u16 = 0x10DE;

fn read<const N: usize>(memory: &dyn InventoryMemory, address: u64) -> Option<[u8; N]> {
    memory.read(address, N).ok()?.try_into().ok()
}

fn pointer(memory: &dyn InventoryMemory, address: u64) -> Option<u64> {
    let value = u64::from_le_bytes(read(memory, address)?);
    (value != 0).then_some(value)
}

fn bank_and_keys(memory: &dyn InventoryMemory, snapshot: &CombatSnapshot) -> Option<(u64, [u8; 16])> {
    let bank = pointer(memory, snapshot.roots.stat.checked_add(0x7D8)?)?;
    Some((bank, read(memory, bank.checked_add(0x7C0)?)?))
}

pub(crate) fn configured_keys(memory: &dyn InventoryMemory, snapshot: &CombatSnapshot) -> Option<[u16; 8]> {
    let first = bank_and_keys(memory, snapshot)?;
    let second = bank_and_keys(memory, snapshot)?;
    let (keys, _) = first.1.as_chunks::<2>();
    (first == second).then(|| std::array::from_fn(|slot| u16::from_le_bytes(keys[slot])))
}

fn water_scale(
    memory: &dyn InventoryMemory,
    snapshot: &CombatSnapshot,
) -> Option<u16> {
    let caster_branch = u32::from_le_bytes(read(memory, snapshot.roots.player.checked_add(0xAB84)?)?);
    if caster_branch != 0 {
        return None;
    }
    let (bank, keys) = bank_and_keys(memory, snapshot)?;
    let mut donors = Vec::new();
    for (slot, key) in keys.as_chunks::<2>().0.iter().enumerate() {
        if u16::from_le_bytes(*key) != SUMMON_WATER {
            continue;
        }
        let row = read::<0xE8>(memory, bank.checked_add(0x80 + 0xE8 * slot as u64)?)?;
        donors.push(PreparedWaterDonor {
            level: u16::from_le_bytes([row[6], row[7]]),
            auxiliary_rank: u16::from_le_bytes([row[0xA], row[0xB]]),
            rarity: row[0x30],
            flags: u32::from_le_bytes([row[0x18], row[0x19], row[0x1A], row[0x1B]]),
        });
    }
    if donors.is_empty() {
        return None;
    }
    let progression = pointer(memory, snapshot.module_base.checked_add(0x47514D0)?)?;
    let holder = pointer(memory, progression.checked_add(8)?)?;
    let [gate] = read(memory, holder.checked_add(0x45)?)?;
    summon_water_scale(&donors, gate)
}

/// A prepared caster recipe is available only when two observations agree.
/// Charge eligibility is checked separately against the bound save bytes.
pub(crate) fn summon_water(memory: &dyn InventoryMemory, snapshot: &CombatSnapshot) -> Option<u16> {
    let first = water_scale(memory, snapshot)?;
    (water_scale(memory, snapshot) == Some(first)).then_some(first)
}
