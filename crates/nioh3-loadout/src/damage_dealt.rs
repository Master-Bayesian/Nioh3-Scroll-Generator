//! PC202 first-contact damage after a successful event, with status 312 initially absent.
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TriggeredDamage {
    NinjaEvade,
    SamuraiDeflect,
    AmritaAbsorption,
    StyleShiftThenAbsorption,
    AbsorptionThenDefense,
    TheoreticalPeak,
}

#[derive(Clone, Copy)]
enum Gate {
    Byte(usize),
    PositiveFloat(usize),
}

impl Gate {
    fn open(self, bank: &[u8]) -> bool {
        match self {
            Self::Byte(at) => bank[at] != 0,
            Self::PositiveFloat(at) => f32::from_bits(crate::pipeline::u32_at(bank, at)) > 0.0,
        }
    }
}

struct Grant {
    gate: Gate,
    factor: f32,
}

/// PC202 class-0 magnitudes at status object offsets A8 and AC.
/// The aggregate applies A8 after comparing the separate virtual strength.
#[derive(Clone, Copy)]
pub(crate) struct DamageOffer {
    factor: f32,
    comparison_factor: f32,
}

impl DamageOffer {
    pub(crate) const fn new(factor: f32, comparison_factor: f32) -> Self {
        Self { factor, comparison_factor }
    }

    pub(crate) const HELD_POWER_PILL: Self = Self::new(1.15, 1.0);

    fn strength(self) -> f32 {
        let comparison = if self.comparison_factor == 1.0 { self.factor } else { self.comparison_factor };
        (comparison - 1.0).abs()
    }

    pub(crate) fn choose(self, later: Self) -> Self {
        if self.strength() < later.strength() { later } else { self }
    }
}

// Native submission order for the sources of 312 in 8437A0, 236BBC4 and 3ED008.
// Each list is already in descending prepared strength. Equal-strength winners
// have identical damage on this contact; duration and later hits are separate.
const EVADE: &[Grant] = &[
    Grant {
        gate: Gate::Byte(0x4ef),
        factor: 1.15,
    }, // 1F6
    Grant {
        gate: Gate::Byte(0xa9f),
        factor: 1.10,
    }, // CB
    Grant {
        gate: Gate::Byte(0x9ed),
        factor: 1.10,
    }, // 31
];
const DEFLECT: &[Grant] = &[
    Grant {
        gate: Gate::Byte(0x4e8),
        factor: 1.15,
    }, // 37D
    Grant {
        gate: Gate::PositiveFloat(0x9f0),
        factor: 1.10,
    }, // 1AE
    Grant {
        gate: Gate::Byte(0xa9f),
        factor: 1.10,
    }, // CB
];
const ABSORB: &[Grant] = &[Grant {
    gate: Gate::Byte(0x3ec),
    factor: 1.10,
}]; // 280

impl TriggeredDamage {
    pub(crate) fn first_hit_factor(self, form: usize, bank: &[u8]) -> Result<f32, &'static str> {
        self.factor_with_supplies(form, bank, None)
    }

    pub(crate) fn factor_with_supplies(
        self,
        form: usize,
        bank: &[u8],
        supply: Option<DamageOffer>,
    ) -> Result<f32, &'static str> {
        if bank.len() <= 0xa9f {
            return Err("incomplete candidate damage bank");
        }
        let (earlier, later): (&[Grant], &[Grant]) = match (self, form) {
            (Self::NinjaEvade, 1) => (&[], EVADE),
            (Self::SamuraiDeflect, 0) => (&[], DEFLECT),
            (Self::AmritaAbsorption | Self::StyleShiftThenAbsorption, 0 | 1) => (&[], ABSORB),
            (Self::AbsorptionThenDefense | Self::TheoreticalPeak, 0) => (ABSORB, DEFLECT),
            (Self::AbsorptionThenDefense | Self::TheoreticalPeak, 1) => (ABSORB, EVADE),
            _ => return Err("damage trigger differs from the active form"),
        };
        let triggered = earlier
            .iter()
            .chain(later)
            .filter(|grant| grant.gate.open(bank))
            .map(|grant| DamageOffer::new(grant.factor, 1.0))
            .fold(DamageOffer::new(1.0, 1.0), DamageOffer::choose);
        let settled = if self == Self::TheoreticalPeak {
            supply.map_or(triggered, |offer| triggered.choose(offer))
        } else {
            triggered
        };
        Ok(settled.factor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absorbs_use_the_equipped_source_and_only_the_damage_channel() {
        let mut bank = [0; 0xb00];
        bank[0x3eb..=0x3ef].fill(1);
        bank[0x3ec] = 0;
        for form in [0, 1] {
            assert_eq!(
                TriggeredDamage::AmritaAbsorption.first_hit_factor(form, &bank),
                Ok(1.0)
            );
        }
        bank[0x3ec] = 1;
        for form in [0, 1] {
            assert_eq!(
                TriggeredDamage::AmritaAbsorption.first_hit_factor(form, &bank),
                Ok(1.10)
            );
        }
        assert_eq!(
            TriggeredDamage::NinjaEvade.first_hit_factor(1, &bank),
            Ok(1.0)
        );
        assert_eq!(
            TriggeredDamage::SamuraiDeflect.first_hit_factor(0, &bank),
            Ok(1.0)
        );
    }

    #[test]
    fn eligible_sources_of_the_same_key_compete_on_the_first_hit() {
        let mut bank = [0; 0xb00];
        bank[0x9f0..0x9f4].copy_from_slice(&1.0f32.to_le_bytes());
        bank[0x9ed] = 1;
        bank[0xa9f] = 1;
        for (event, form, stronger) in [
            (TriggeredDamage::NinjaEvade, 1, 0x4ef),
            (TriggeredDamage::SamuraiDeflect, 0, 0x4e8),
        ] {
            assert_eq!(event.first_hit_factor(form, &bank), Ok(1.10));
            bank[stronger] = 1;
            assert_eq!(event.first_hit_factor(form, &bank), Ok(1.15));
            assert!(event.first_hit_factor(1 - form, &bank).is_err());
        }
    }

    #[test]
    fn first_contact_accepts_the_original_prefix_and_padded_banks() {
        let bank = [0u8; 0xb20];
        let event = TriggeredDamage::NinjaEvade;
        assert!(event.first_hit_factor(1, &bank[..0xa9f]).is_err());
        assert_eq!(event.first_hit_factor(1, &bank[..0xaa0]), Ok(1.0));
        assert_eq!(event.first_hit_factor(1, &bank), Ok(1.0));
    }

    #[test]
    fn settled_absorption_and_defense_apply_one_winning_damage_factor() {
        for (form, weak, strong) in [(0, 0xa9f, 0x4e8), (1, 0x9ed, 0x4ef)] {
            for (absorb, defense, stronger, expected) in [
                (0, 0, 0, 1.0),
                (1, 0, 0, 1.10),
                (0, 1, 0, 1.10),
                (1, 1, 0, 1.10),
                (1, 1, 1, 1.15),
            ] {
                let mut bank = [0u8; 0xb00];
                bank[0x3ec] = absorb;
                bank[weak] = defense;
                bank[strong] = stronger;
                assert_eq!(
                    TriggeredDamage::AbsorptionThenDefense.first_hit_factor(form, &bank),
                    Ok(expected)
                );
            }
        }
    }

    #[test]
    fn held_pill_competes_in_the_native_damage_group() {
        let mut bank = [0u8; 0xb00];
        let peak = TriggeredDamage::TheoreticalPeak;
        for (form, strong) in [(0, 0x4e8), (1, 0x4ef)] {
            bank.fill(0);
            let pill = Some(DamageOffer::HELD_POWER_PILL);
            assert_eq!(peak.factor_with_supplies(form, &bank, None), Ok(1.0));
            assert_eq!(peak.factor_with_supplies(form, &bank, pill), Ok(1.15));
            bank[0x3ec] = 1;
            assert_eq!(peak.factor_with_supplies(form, &bank, None), Ok(1.10));
            assert_eq!(peak.factor_with_supplies(form, &bank, pill), Ok(1.15));
            bank[strong] = 1;
            assert_eq!(peak.factor_with_supplies(form, &bank, pill), Ok(1.15));
            assert_eq!(
                TriggeredDamage::AmritaAbsorption.factor_with_supplies(form, &bank, pill),
                Ok(1.10)
            );
        }
    }

    #[test]
    fn class_zero_compares_strength_separately_from_the_applied_factor() {
        let pill = DamageOffer::HELD_POWER_PILL;
        assert_eq!(pill.choose(DamageOffer::new(1.40, 1.10)).factor, 1.15);
        assert_eq!(pill.choose(DamageOffer::new(1.4826, 1.40)).factor, 1.4826);
        assert_eq!(pill.choose(DamageOffer::new(1.40, 1.15)).factor, 1.15);
    }
}
