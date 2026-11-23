//! PC202 prepared item magnitudes for the self-cast PvE branch.

use crate::damage_dealt::DamageOffer;

include!("peak_prepared_tables.rs");

const WIDE_ITEM_COUNT: u32 = 0x0020_0000;

pub(crate) fn jutsu_power(stat: &[u8]) -> Option<u32> {
    let word = stat.get(0x1c4..0x1c8)?.try_into().ok()?;
    Some(u32::from_le_bytes(word))
}

/// 10DE invokes 929D/opcode 498, which prepares source 155 with a null
/// source actor. Original 3EFB90 reads the recipient's selected STAT+1C4.
/// The separate incoming penalty (source 172) never enters this caster's
/// outgoing damage factor.
pub(crate) fn carnage_damage(caster_stat: u32) -> DamageOffer {
    let base = 400.0_f32 * 0.001;
    let power = (2.0_f32 * 0.01 * 0.01) * caster_stat as f32;
    DamageOffer::new(1.0 + (base + power), 1.0 + base)
}

/// The native donor record from a prepared slot whose item key is `0xCBC3`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedWaterDonor {
    /// The little-endian u16 at donor offset `+0x06`.
    pub level: u16,
    /// The little-endian u16 at donor offset `+0x0A`.
    pub auxiliary_rank: u16,
    /// The native byte at donor offset `+0x30`.
    pub rarity: u8,
    /// The little-endian u32 at donor offset `+0x18`.
    pub flags: u32,
}

/// Returns the original scalar in thousandths for the prepared donors in slot order.
///
/// Supply readiness and remaining uses are decided by the caller. The `gate`
/// argument is the byte read by the PC202 native auxiliary-rank cap.
pub fn summon_water_scale(donors: &[PreparedWaterDonor], gate: u8) -> Option<u16> {
    if donors.is_empty() || donors.len() > 8 {
        return None;
    }

    let auxiliary_limit = match gate {
        2 => 15,
        3 => 30,
        4 => 45,
        5 => 120,
        _ => 5,
    };
    let mut best_priority = 0;
    let mut selected_level = 1;
    let mut selected_auxiliary = 0;
    for donor in donors {
        let level = if donor.flags & WIDE_ITEM_COUNT == 0 {
            donor.level.min(180)
        } else {
            0
        };
        let rarity_bonus = RARITY_LEVEL_BONUS
            .get(usize::from(donor.rarity))
            .copied()
            .unwrap_or(0);
        let auxiliary = donor.auxiliary_rank.min(auxiliary_limit);
        let lower_rank = auxiliary.min(99);
        let auxiliary_priority = 10 * lower_rank + 3 * (auxiliary - lower_rank);
        let priority = level + rarity_bonus + auxiliary_priority;
        if priority > best_priority {
            best_priority = priority;
            selected_level = (level + rarity_bonus).min(190);
            selected_auxiliary = auxiliary;
        }
    }

    // PC202 optional multiplier AEAC adds 3 to the second auxiliary cap.
    // The original donor selection already imposed the tighter gate cap.
    Some(
        BASE_SCALE_MILLI[usize::from(selected_level)]
            .wrapping_add(AUXILIARY_SCALE_MILLI[usize::from(selected_auxiliary)]),
    )
}

/// Converts CBC3's prepared scale and the candidate's `STAT+0x1C4` to Water power.
///
/// The original opcode 4 uses base power 30 and the self-cast PvE caster column.
pub fn summon_water_power(scale_milli: u16, caster_stat: u32) -> u32 {
    let scale = f32::from(scale_milli) * 0.001;
    let scaled_base = (30.0 * scale) as u32;
    let caster_bonus = (caster_stat as f32 * 0.01) as u32;
    scaled_base + caster_bonus
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{damage_dealt::TriggeredDamage, pipeline::source_rebuild::PeakSupplies};

    #[test]
    fn prepared_carnage_competes_with_the_pill_and_rebuilds_with_jutsu_power() {
        let mut stat = [0; 0x1c8];
        let mut damage = [0; 0xaa0];
        damage[0x3ec] = 1;
        let supplies = PeakSupplies { held_power_pill: true, prepared_carnage: true, ..Default::default() };
        let peak = TriggeredDamage::TheoreticalPeak;
        for (power, expected) in [(0_u32, 0x3fb3_3333), (413, 0x3fbd_c5d6), (635, 0x3fc3_74bc)] {
            stat[0x1c4..0x1c8].copy_from_slice(&power.to_le_bytes());
            let grant = supplies.damage_offer(&stat).unwrap();
            assert_eq!(peak.factor_with_supplies(0, &damage, grant).unwrap().to_bits(), expected);
        }
        assert_eq!(TriggeredDamage::AmritaAbsorption.factor_with_supplies(0, &damage, supplies.damage_offer(&stat).unwrap()), Ok(1.10));
    }

    #[test]
    fn retained_cbc3_uses_the_candidate_caster_stat() {
        let donor = PreparedWaterDonor {
            level: 170,
            auxiliary_rank: 4,
            rarity: 3,
            flags: 0x84,
        };
        let scale = summon_water_scale(&[donor], 3).unwrap();
        assert_eq!(scale, 1258);
        assert_eq!(summon_water_power(scale, 0), 37);
        assert_eq!(summon_water_power(scale, 635), 43);
        assert_eq!(summon_water_power(scale, 700), 44);
    }

    #[test]
    fn prepared_donor_priority_preserves_the_first_slot_on_ties() {
        let earlier = PreparedWaterDonor {
            level: 170,
            auxiliary_rank: 4,
            rarity: 3,
            flags: 0,
        };
        let later = PreparedWaterDonor {
            level: 160,
            auxiliary_rank: 5,
            ..earlier
        };
        let earlier_scale = summon_water_scale(&[earlier], 3);
        let later_scale = summon_water_scale(&[later], 3);
        assert_ne!(earlier_scale, later_scale);
        assert_eq!(summon_water_scale(&[earlier, later], 3), earlier_scale);
        assert_eq!(summon_water_scale(&[later, earlier], 3), later_scale);
        assert_eq!(summon_water_scale(&[], 3), None);
    }

    #[test]
    fn native_priority_handles_wide_counts_and_the_auxiliary_breakpoint() {
        let high_rank = PreparedWaterDonor {
            level: 160,
            auxiliary_rank: 100,
            rarity: 0,
            flags: 0,
        };
        let lower_rank = PreparedWaterDonor {
            level: 166,
            auxiliary_rank: 99,
            ..high_rank
        };
        assert_eq!(summon_water_scale(&[high_rank, lower_rank], 5), Some(1248));

        let wide = PreparedWaterDonor {
            level: 170,
            auxiliary_rank: 4,
            rarity: 3,
            flags: 0x0020_0084,
        };
        assert_eq!(summon_water_scale(&[wide], 3), Some(1003));

        let no_priority = PreparedWaterDonor {
            level: 0,
            auxiliary_rank: 0,
            ..high_rank
        };
        assert_eq!(summon_water_scale(&[no_priority], 3), Some(1000));
    }
}
