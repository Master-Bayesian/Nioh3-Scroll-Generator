//! Ordinary Ninja kusarigama Reaper depletion for the resolved PC 2.0.2 action branch.
//! Inputs come from candidate STAT/D. No observed iteration count is an input.

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn float(bytes: &[u8], at: usize) -> f32 {
    f32::from_bits(word(bytes, at))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeldPhase {
    Entry,
    FirstLoop,
    SecondLoop,
    Exhausted,
}
impl HeldPhase {
    pub fn hit_index(self) -> Option<usize> {
        match self {
            Self::Entry => Some(0),
            Self::FirstLoop => Some(1),
            Self::SecondLoop => Some(2),
            Self::Exhausted => None,
        }
    }
    pub fn after_debit(self, ki: f32) -> Self {
        if ki <= 0.0 {
            return Self::Exhausted;
        }
        match self {
            Self::Entry | Self::SecondLoop => Self::FirstLoop,
            Self::FirstLoop => Self::SecondLoop,
            Self::Exhausted => Self::Exhausted,
        }
    }
}

pub fn reaper_steps(stat: &[u8], d: &[u8]) -> Result<u32, &'static str> {
    walk_reaper(stat, d, |_| {})
}

/// Per-contact resource state for original-machine replay, after the action debit.
pub fn reaper_ki_after_hits(stat: &[u8], d: &[u8]) -> Result<Vec<u32>, &'static str> {
    let mut values = Vec::new();
    walk_reaper(stat, d, |ki| values.push(ki.to_bits()))?;
    Ok(values)
}

/// Candidate-owned ordinary debit inputs. This is the proven Ninja Reaper
/// branch, not a promise that every action shares its predicates or curves.
#[derive(Clone, Copy)]
struct OrdinaryReaperCost {
    prefix: f32,
    class: f32,
    low_ki_curve: bool,
}
impl OrdinaryReaperCost {
    fn bind(stat: &[u8], d: &[u8]) -> Result<Self, &'static str> {
        if stat.len() < 0x7a0 || d.len() < 0x814 {
            return Err("ordinary Ki producer inputs are incomplete");
        }
        let score = word(stat, 0x79C);
        let class = if d[0x6D1] != 0 && score >= 85 {
            0x3f170a3d
        } else if score >= 75 {
            0x3f2e147b
        } else if score >= 25 {
            0x3f3851eb
        } else if score != 0 {
            0x3f733333
        } else {
            0x40100000
        };
        let mut prefix = 1.0 - float(d, 0x160);
        if float(d, 0x7F8) > 0.0 {
            prefix *= 1.0 - float(d, 0x7F8);
        }
        prefix *= (float(d, 0x810) + 1.0) * (1.0 - float(d, 0x170));
        if d[0x41C] != 0 {
            prefix *= f32::from_bits(0x3f599999);
        }
        if d[0x6D0] != 0 {
            prefix *= 1.0 - 20.0f32 * 0.01f32;
        }
        if !prefix.is_finite() {
            return Err("nonfinite ordinary Ki prefix");
        }
        Ok(Self {
            prefix,
            class: f32::from_bits(class),
            low_ki_curve: (word(d, 0x2AC) as i32) >= 0,
        })
    }
    fn debit(self, base: f32, ki: f32, maximum: f32) -> Result<f32, &'static str> {
        if !base.is_finite() || !ki.is_finite() || !maximum.is_finite() || maximum <= 0.0 {
            return Err("invalid ordinary Ki debit state");
        }
        let coordinate = ((1.0f32 - ki / maximum) * 100.0f32) as i32;
        let percent = if coordinate <= 50 {
            5.0
        } else if coordinate >= 80 {
            40.0
        } else {
            let (x, left, right) = if coordinate <= 60 {
                (50, 5.0f32, 15.0f32)
            } else if coordinate <= 70 {
                (60, 15.0, 30.0)
            } else {
                (70, 30.0, 40.0)
            };
            left + ((coordinate - x) as f32 / 10.0f32) * (right - left)
        };
        let reduction = if self.low_ki_curve {
            percent / 100.0f32
        } else {
            0.0
        };
        let factor = ((self.prefix * (1.0f32 - reduction)) * self.class).max(0.0);
        let debit = factor * base;
        if !debit.is_finite() {
            return Err("nonfinite ordinary Ki debit");
        }
        Ok(debit)
    }
}

pub fn ordinary_reaper_cost(
    stat: &[u8],
    d: &[u8],
    base: f32,
    ki: f32,
    maximum: f32,
) -> Result<f32, &'static str> {
    OrdinaryReaperCost::bind(stat, d)?.debit(base, ki, maximum)
}

fn walk_reaper(stat: &[u8], d: &[u8], mut after_hit: impl FnMut(f32)) -> Result<u32, &'static str> {
    let cost = OrdinaryReaperCost::bind(stat, d)?;
    let maximum = word(stat, 0x1C) as f32;
    use crate::sequence::{walk_sequence, ActionStep, ResolvedSequence};
    // Bound resource23c75bb8...31d86a: action-parameter i16+16.
    let entry = [ActionStep {
        action: 25192,
        base_cost: 230.0,
        contacts: &[0],
    }];
    let cycle = [
        ActionStep {
            action: 25193,
            base_cost: 115.0,
            contacts: &[1],
        },
        ActionStep {
            action: 25194,
            base_cost: 115.0,
            contacts: &[2],
        },
    ];
    walk_sequence(
        ResolvedSequence {
            entry: &entry,
            cycle: &cycle,
            cycles: None,
        },
        maximum,
        |base, ki, maximum| cost.debit(base, ki, maximum),
        |hit| {
            after_hit(hit.ki_after);
            Ok(())
        },
    )
}

pub fn sum_reaper(hits: [u32; 3], steps: u32) -> u32 {
    let mut total = 0.0f32;
    for step in 0..steps {
        let index = if step == 0 {
            0
        } else if step % 2 == 1 {
            1
        } else {
            2
        };
        total += f32::from_bits(hits[index]);
    }
    total.to_bits()
}

pub fn sum_reaper_checked(hits: [u32; 3], steps: u32) -> Result<u32, &'static str> {
    let result = sum_reaper(hits, steps);
    if f32::from_bits(result).is_finite() {
        Ok(result)
    } else {
        Err("held contact sum is nonfinite")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_contacts_do_not_hide_a_nonfinite_debit_contract() {
        let stat = vec![0u8; 0x900];
        let mut d = vec![0u8; 0xB00];
        d[0x160..0x164].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
        assert!(reaper_steps(&stat, &d).is_err());
        d[0x160..0x164].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(reaper_steps(&stat, &d), Ok(0));
    }
    #[test]
    fn repeated_finite_contacts_cannot_publish_an_infinite_score() {
        let hits = [1.0e38f32.to_bits(); 3];
        assert!(sum_reaper_checked(hits, 3).is_ok());
        assert!(sum_reaper_checked(hits, 4).is_err());
        assert_eq!(sum_reaper_checked([f32::NAN.to_bits(); 3], 0), Ok(0));
        assert_eq!(
            sum_reaper_checked([1.0f32.to_bits(), f32::NAN.to_bits(), 0], 1),
            Ok(1.0f32.to_bits())
        );
    }
}
