//! Read-only PC v2.02 configuration projection. Unknown semantics never become defaults.
use crate::character::{tagged_value_offset, EQUIPMENT_SET_OFFSETS};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SavedSkill { pub id: u16, pub stored_rank: u16, pub selected_rank: u16 }
#[derive(Debug, Serialize)]
pub struct SavedClan { pub group_id: u16, pub raw: u32, pub clan_id: u32, pub effect_index: u32 }
#[derive(Debug, Serialize)]
pub struct SavedGuardian { pub id: u32, pub level: u32, pub owned: bool }
#[derive(Debug, Serialize)]
pub struct SavedClanMembership { pub clan_id: u32, pub cached_rank_index: u32 }
#[derive(Debug, Serialize)]
pub struct SavedLoadout {
    pub profile: &'static str,
    pub complete: bool,
    pub attributes: Option<Vec<u32>>,
    pub initial_attributes: Option<Vec<u32>>,
    pub level: Option<u32>,
    pub capacities: Option<Vec<u32>>,
    pub skills: Option<Vec<SavedSkill>>,
    pub clan_protections: Option<Vec<SavedClan>>,
    pub guardian_ownership: Option<Vec<SavedGuardian>>,
    pub guardian_selection: Option<Vec<u32>>,
    pub clan_membership: Option<SavedClanMembership>,
    pub equipped_indices: Option<Vec<Vec<u32>>>,
    pub soul_core_indices: Option<Vec<u32>>,
    pub destroyed_spike_count: Option<u16>,
    pub unavailable: Vec<&'static str>,
}

fn payload(plain: &[u8], key: u32, width: usize) -> Result<&[u8], ()> {
    let offset = tagged_value_offset(plain, key, width).map_err(|_| ())?;
    plain.get(offset..offset + width).ok_or(())
}
fn u32_at(value: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([value[offset], value[offset + 1], value[offset + 2], value[offset + 3]])
}
fn u16_at(value: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([value[offset], value[offset + 1]])
}
fn array(plain: &[u8], key: u32, count: usize, stride: usize) -> Result<&[u8], ()> {
    let bytes = payload(plain, key, 4 + count * stride)?;
    if u32_at(bytes, 0) != count as u32 { return Err(()); }
    Ok(&bytes[4..])
}
fn field(row: &[u8], offset: usize, key: u32, width: usize) -> Result<&[u8], ()> {
    let header = row.get(offset..offset + 8).ok_or(())?;
    if u32_at(header, 0) != key || u32_at(header, 4) != width as u32 { return Err(()); }
    row.get(offset + 8..offset + 8 + width).ok_or(())
}
fn read_attributes(plain: &[u8], key: u32) -> Result<Vec<u32>, ()> {
    Ok(array(plain, key, 7, 4)?.chunks_exact(4).map(|v| u32_at(v, 0)).collect())
}
fn capacities(plain: &[u8]) -> Result<Vec<u32>, ()> {
    array(plain, 0x6EEA1A44, 3, 24)?.chunks_exact(24).map(|row| {
        let base = u32_at(field(row, 0, 0x4404FE9A, 4)?, 0);
        let added = u32_at(field(row, 12, 0x68BAC844, 4)?, 0);
        base.checked_add(added).ok_or(())
    }).collect()
}
fn skills(plain: &[u8]) -> Result<Vec<SavedSkill>, ()> {
    array(plain, 0x9BB231A2, 2000, 64)?.chunks_exact(64).map(|row| {
        let id = u16_at(field(row, 0, 0xE6A3A9B0, 2)?, 0);
        field(row, 10, 0xCC03F7B1, 1)?;
        field(row, 19, 0x5E296F9C, 1)?;
        let stored_rank = u16_at(field(row, 28, 0xC573E184, 2)?, 0);
        let selected_rank = u16_at(field(row, 38, 0x5EA83962, 2)?, 0);
        field(row, 48, 0x0474B37D, 8)?;
        Ok(SavedSkill { id, stored_rank, selected_rank })
    }).collect()
}
fn clans(plain: &[u8]) -> Result<Vec<SavedClan>, ()> {
    array(plain, 0x32AF52F6, 4, 46)?.chunks_exact(46).map(|row| {
        Ok(SavedClan {
            group_id: u16_at(field(row, 0, 0xBA6279E7, 2)?, 0),
            raw: u32_at(field(row, 10, 0x9FBCCAB5, 4)?, 0),
            clan_id: u32_at(field(row, 22, 0x15C9A99D, 4)?, 0),
            effect_index: u32_at(field(row, 34, 0x512062D6, 4)?, 0),
        })
    }).collect()
}
fn guardians(plain: &[u8]) -> Result<Vec<SavedGuardian>, ()> {
    array(plain, 0x618D6712, 128, 67)?.chunks_exact(67).map(|row| {
        let id = u32_at(field(row, 0, 0x9EC71EC4, 4)?, 0);
        field(row, 12, 0x31F58C89, 8)?;
        let level = u32_at(field(row, 28, 0x7E167570, 4)?, 0);
        let owned = field(row, 40, 0xB412C027, 1)?[0];
        field(row, 49, 0xD8BD6F8C, 1)?;
        field(row, 58, 0x99641529, 1)?;
        if owned > 1 { return Err(()); }
        Ok(SavedGuardian { id, level, owned: owned == 1 })
    }).collect()
}
fn equipped_indices(plain: &[u8]) -> Result<Vec<Vec<u32>>, ()> {
    array(plain, 0x6B52DB66, 2, 80)?.chunks_exact(80).map(|row| {
        let values = field(row, 0, 0x54B78304, 72)?;
        if u32_at(values, 0) != 17 { return Err(()); }
        Ok(values[4..].chunks_exact(4).map(|v| u32_at(v, 0)).collect())
    }).collect()
}
fn core_indices(plain: &[u8]) -> Result<Vec<u32>, ()> {
    array(plain, 0xAB0E9943, 10, 12)?.chunks_exact(12).map(|row| {
        Ok(u32_at(field(row, 0, 0x36BFE4B6, 4)?, 0))
    }).collect()
}
fn capture<T>(result: Result<T, ()>, name: &'static str, unavailable: &mut Vec<&'static str>) -> Option<T> {
    match result { Ok(value) => Some(value), Err(_) => { unavailable.push(name); None } }
}
pub fn read_loadout(plain: &[u8]) -> SavedLoadout {
    let mut unavailable = Vec::new();
    let attributes = capture(read_attributes(plain, 0x607E11FF), "attributes", &mut unavailable);
    let initial_attributes = capture(read_attributes(plain, 0x614D187E), "initial_attributes", &mut unavailable);
    let level = capture(payload(plain, 0x5F81D21B, 4).map(|v| u32_at(v, 0)), "level", &mut unavailable);
    let capacities = capture(capacities(plain), "capacities", &mut unavailable);
    let skills = capture(skills(plain), "skills", &mut unavailable);
    let clan_protections = capture(clans(plain), "clan_protections", &mut unavailable);
    let guardian_ownership = capture(guardians(plain), "guardian_ownership", &mut unavailable);
    let guardian_selection = capture(
        array(plain, 0x3520992B, 2, 4).map(|rows| rows.chunks_exact(4).map(|v| u32_at(v, 0)).collect()),
        "guardian_selection", &mut unavailable);
    // This cache belongs only to the currently joined clan, not all protection choices.
    let clan_membership = capture((|| {
        Ok(SavedClanMembership {
            clan_id: u32_at(payload(plain, 0x10316BAB, 4)?, 0),
            cached_rank_index: u32_at(payload(plain, 0x3C37295A, 4)?, 0),
        })
    })(), "clan_membership", &mut unavailable);
    let destroyed_spike_count = capture(payload(plain, 0x2441B18A, 2).map(|v| u16_at(v, 0)), "destroyed_spike_count", &mut unavailable);
    let equipped_indices = capture(equipped_indices(plain), "equipped_indices", &mut unavailable);
    let soul_core_indices = capture(core_indices(plain), "soul_core_indices", &mut unavailable);
    SavedLoadout { profile: "pc_v2_02", complete: unavailable.is_empty(), attributes, initial_attributes, level, capacities,
        skills, clan_protections, guardian_ownership, guardian_selection, clan_membership,
        equipped_indices, soul_core_indices, destroyed_spike_count, unavailable }
}
pub fn equipment_positions(record: &[u8]) -> Option<[u32; 2]> {
    Some([u32_at(record.get(EQUIPMENT_SET_OFFSETS[0]..EQUIPMENT_SET_OFFSETS[0] + 4)?, 0),
        u32_at(record.get(EQUIPMENT_SET_OFFSETS[1]..EQUIPMENT_SET_OFFSETS[1] + 4)?, 0)])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tag(key: u32, value: &[u8]) -> Vec<u8> {
        [key.to_le_bytes().as_slice(), (value.len() as u32).to_le_bytes().as_slice(), value].concat()
    }
    #[test]
    fn missing_or_ambiguous_fields_do_not_supply_defaults() {
        let mut bytes = tag(0x5F81D21B, &246u32.to_le_bytes());
        assert_eq!(read_loadout(&bytes).level, Some(246));
        bytes.extend(tag(0x5F81D21B, &1u32.to_le_bytes()));
        let result = read_loadout(&bytes);
        assert_eq!(result.level, None);
        assert!(result.unavailable.contains(&"level"));
        assert!(!result.complete);
    }
    #[test]
    fn native_array_headers_and_capacity_overflow_are_checked() {
        let values: Vec<u8> = [7u32, 5, 5, 5, 6, 5, 5, 5].into_iter().flat_map(u32::to_le_bytes).collect();
        let bytes = tag(0x607E11FF, &values);
        assert_eq!(read_loadout(&bytes).attributes.unwrap(), vec![5, 5, 5, 6, 5, 5, 5]);
        let mut rows = 3u32.to_le_bytes().to_vec();
        for _ in 0..3 { rows.extend(tag(0x4404FE9A, &10u32.to_le_bytes())); rows.extend(tag(0x68BAC844, &32u32.to_le_bytes())); }
        assert_eq!(read_loadout(&tag(0x6EEA1A44, &rows)).capacities, Some(vec![42; 3]));
        rows[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(read_loadout(&tag(0x6EEA1A44, &rows)).capacities, None);
    }
    #[test]
    fn malformed_record_field_order_is_rejected() {
        let rows = [4u32.to_le_bytes().as_slice(), &[0u8; 184]].concat();
        assert_eq!(read_loadout(&tag(0x32AF52F6, &rows)).clan_protections.as_ref().map(Vec::len), None);
        assert_eq!(equipment_positions(&[0u8; 10]), None);
    }
    #[test]
    fn selected_guardians_are_separate_from_ownership_and_clan_cache_is_not_all_ranks() {
        let ids = [2u32, 3, 350].into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>();
        let mut plain = tag(0x3520992B, &ids);
        plain.extend(tag(0x10316BAB, &7u32.to_le_bytes()));
        plain.extend(tag(0x3C37295A, &0u32.to_le_bytes()));
        let result = read_loadout(&plain);
        assert_eq!(result.guardian_selection, Some(vec![3, 350]));
        assert!(!result.unavailable.contains(&"guardian_selection"));
        assert_eq!(result.clan_membership.unwrap().clan_id, 7);
        assert!(!result.complete);
        assert!(result.guardian_ownership.is_none());
        plain.extend(tag(0x3520992B, &ids));
        assert_eq!(read_loadout(&plain).guardian_selection, None);
    }
    #[test]
    fn equipment_and_core_indices_keep_native_positions_and_reject_bad_children() {
        let mut sets = 2u32.to_le_bytes().to_vec();
        for _ in 0..2 {
            let values = [vec![17u32], vec![u32::MAX; 17]].concat().into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>();
            sets.extend(tag(0x54B78304, &values));
        }
        let mut cores = 10u32.to_le_bytes().to_vec();
        for index in 0..10 { cores.extend(tag(0x36BFE4B6, &(index as u32).to_le_bytes())); }
        let plain = [tag(0x6B52DB66, &sets), tag(0xAB0E9943, &cores)].concat();
        let result = read_loadout(&plain);
        assert_eq!(result.equipped_indices.unwrap(), vec![vec![u32::MAX; 17]; 2]);
        assert_eq!(result.soul_core_indices.unwrap(), (0..10).collect::<Vec<u32>>());
        sets[4] ^= 1;
        assert!(read_loadout(&tag(0x6B52DB66, &sets)).equipped_indices.is_none());
        cores[8..12].copy_from_slice(&8u32.to_le_bytes());
        assert!(read_loadout(&tag(0xAB0E9943, &cores)).soul_core_indices.is_none());
    }
}
