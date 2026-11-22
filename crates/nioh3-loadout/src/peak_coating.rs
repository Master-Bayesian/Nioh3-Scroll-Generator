//! Held PC202 amulets for an admitted direct actor contact.

use serde::{Deserialize, Serialize};

/// Temporary weapon element selected for a peak cast.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HeldCoating {
    Fire,
    Water,
    Lightning,
    Wind,
}

impl HeldCoating {
    /// Order shared with the held coating stock vector.
    pub const ALL: [Self; 4] = [Self::Fire, Self::Water, Self::Lightning, Self::Wind];

    pub const fn stock_index(self) -> usize {
        self as usize
    }

    pub const fn item_id(self) -> u16 {
        match self {
            Self::Fire => 0xF481,
            Self::Water => 0x304E,
            Self::Lightning => 0xC389,
            Self::Wind => 0x72BC,
        }
    }

    pub const fn native_selector(self) -> u32 {
        match self {
            Self::Fire => 0,
            Self::Water => 1,
            Self::Lightning => 2,
            Self::Wind => 22,
        }
    }

    pub const fn power(self) -> u32 {
        30
    }

    /// Set the established temporary-element lanes for a compatible direct
    /// actor template whose packet address and flags have already been bound.
    pub const fn packet_words(self, main: u64, original_flags: u32, power: u32) -> [(u64, u32); 3] {
        [
            (main + 0x14, self.native_selector()),
            (main + 0x18, power),
            (main + 0x13C, original_flags | (1 << 28)),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::HeldCoating;
    use crate::pipeline::source_rebuild::PeakSupplies;

    #[test]
    fn original_amulet_ids_and_packet_words() {
        for (index, (coating, (item_id, selector))) in HeldCoating::ALL
            .into_iter()
            .zip([(0xF481, 0), (0x304E, 1), (0xC389, 2), (0x72BC, 22)])
            .enumerate()
        {
            assert_eq!(coating.stock_index(), index);
            assert_eq!(coating.item_id(), item_id);
            assert_eq!(
                coating.packet_words(0x1000, 0x0200_1040, coating.power()),
                [(0x1014, selector), (0x1018, 30), (0x113C, 0x1200_1040)]
            );
        }
    }

    #[test]
    fn eligible_coating_sources_choose_one_power_for_the_requested_element() {
        let stock = PeakSupplies {
            held_power_pill: true,
            held_coatings: [true, true, false, false],
            prepared_water_scale: Some(1258),
            prepared_carnage: false,
            prepared_weakness: false,
        };
        assert_eq!(stock.coating_power(HeldCoating::Water, 635), Some(43));
        assert_eq!(stock.coating_power(HeldCoating::Water, 700), Some(44));
        assert_eq!(stock.coating_power(HeldCoating::Fire, 700), Some(30));
        assert_eq!(stock.coating_power(HeldCoating::Lightning, 700), None);

        let prepared_only = PeakSupplies { held_coatings: [false; 4], ..stock };
        assert!(prepared_only.can_coat(HeldCoating::Water));
        assert_eq!(prepared_only.coating_power(HeldCoating::Water, 635), Some(43));
        let weaker = PeakSupplies { prepared_water_scale: Some(500), ..stock };
        assert_eq!(weaker.coating_power(HeldCoating::Water, 635), Some(30));
    }
}
