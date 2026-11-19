//! STAT merge semantics follow writers, never the magnitude of their bit patterns.
//! Float residuals remain an explicitly scoped captured-baseline operation; this
//! module does not claim to recover missing upstream producers by subtraction.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StatWord {
    Rebuilt,
    Add32,
    AddLow16,
    FloatResidual,
    Unchanged,
}

pub(crate) fn kind(at: usize) -> StatWord {
    // 818248 constructs five 0x40-byte records, including categorical tags,
    // sentinels and byte flags. These are assignments, not additive quantities.
    if (0x24..0x164).contains(&at) || (0x1F4..0x218).contains(&at) {
        return StatWord::Rebuilt;
    }
    match at {
        0x18 | 0x1C | 0x164 | 0x168 | 0x16C | 0x174 | 0x178 | 0x1BC | 0x1C0
        | 0x1C4 | 0x1C8 | 0x218 => StatWord::Add32,
        0x21C => StatWord::AddLow16,
        0x1A8 | 0x1AC | 0x4DC | 0x4FC | 0x51C | 0x53C | 0x55C | 0x57C
        | 0x77C | 0x780 | 0x784 | 0x788 | 0x78C | 0x790 => StatWord::FloatResidual,
        _ => StatWord::Unchanged,
    }
}

pub(crate) fn merge(at: usize, baseline: u32, candidate: u32, captured: u32) -> Result<u32, &'static str> {
    if kind(at) == StatWord::Rebuilt {
        return Ok(candidate);
    }
    if baseline == candidate {
        return Ok(captured);
    }
    match kind(at) {
        StatWord::Rebuilt => Ok(candidate),
        StatWord::Add32 => Ok(captured.wrapping_add(candidate).wrapping_sub(baseline)),
        StatWord::AddLow16 => {
            if (baseline ^ candidate) & 0xFFFF_0000 != 0 {
                return Err("unclassified write to STAT+21E");
            }
            let low = (captured as u16).wrapping_add(candidate as u16).wrapping_sub(baseline as u16);
            Ok((captured & 0xFFFF_0000) | u32::from(low))
        }
        StatWord::FloatResidual => {
            let (a, b, c) = (f32::from_bits(baseline), f32::from_bits(candidate), f32::from_bits(captured));
            if !a.is_finite() || !b.is_finite() || !c.is_finite() {
                return Err("nonfinite STAT float residual input");
            }
            let value = (f64::from(c) + f64::from(b) - f64::from(a)) as f32;
            if !value.is_finite() {
                return Err("STAT float residual overflow");
            }
            Ok(value.to_bits())
        }
        StatWord::Unchanged => Err("changed STAT word has no writer-derived merge contract"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categorical_sentinel_is_a_replacement_for_every_weapon_record() {
        for i in 0..5 {
            assert_eq!(merge(0x3C + i * 64, u32::MAX, 2, u32::MAX), Ok(2));
            assert_eq!(merge(0x3C + i * 64, u32::MAX, 2, 3), Ok(2));
            assert_eq!(merge(0x3C + i * 64, u32::MAX, u32::MAX, 3), Ok(u32::MAX));
            assert_eq!(merge(0x60 + i * 64, 0x0000_0100, 1, 0x0000_0100), Ok(1));
        }
    }

    #[test]
    fn integer_words_do_not_turn_into_floats_at_24_bits() {
        assert_eq!(merge(0x1C8, 0x0100_0000, 0x0100_0001, 0), Ok(1));
        assert_eq!(merge(0x218, u32::MAX, 0, 7), Ok(8));
    }

    #[test]
    fn packed_add16_does_not_borrow_from_its_neighbor() {
        assert_eq!(merge(0x21C, 0x0007_FFFE, 0x0007_0001, 0x0007_000A), Ok(0x0007_000D));
        assert!(merge(0x21C, 0x0007_0000, 0x0008_0000, 0).is_err());
    }

    #[test]
    fn unknown_writers_and_nonfinite_residuals_are_explicit_failures() {
        assert!(merge(0x220, 0, 1, 0).is_err());
        assert!(merge(0x77C, 0, f32::NAN.to_bits(), 0).is_err());
        assert_eq!(merge(0x77C, 5.0f32.to_bits(), 7.0f32.to_bits(), 10.0f32.to_bits()), Ok(12.0f32.to_bits()));
    }
}
