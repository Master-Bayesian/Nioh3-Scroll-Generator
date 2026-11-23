//! PC202 Samurai Arts Proficiency actor object (definition 7011, type 36D8).
use serde::{Deserialize, Deserializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Start {
    Current,
    Untriggered,
    FullGaugeActivation,
    TheoreticalPeak,
}

/// Earlier PC202 event plans used a boolean named current_effect.
pub fn read_start<'de, D: Deserializer<'de>>(decoder: D) -> Result<Start, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Input {
        Named(Start),
        Legacy(bool),
    }
    Ok(match Input::deserialize(decoder)? {
        Input::Named(start) => start,
        Input::Legacy(true) => Start::Current,
        Input::Legacy(false) => Start::Untriggered,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KiPhase {
    Ready,
    Debited,
    Charging,
}

impl KiPhase {
    pub fn from_object(value: u32) -> Self {
        match value {
            0 => Self::Ready,
            1 => Self::Debited,
            _ => Self::Charging,
        }
    }

    pub(crate) fn encoded(self) -> u8 {
        match self {
            Self::Ready => 0,
            Self::Debited => 1,
            Self::Charging => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveArts {
    pub distinct_keys: u64,
    pub owner_additive_bits: u32,
    pub ki_phase: KiPhase,
}

/// PC202 35A220's two independently selected Arts channels.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreparedActorRoute {
    pub(crate) arts_eligible: bool,
    pub(crate) follow_up_eligible: bool,
}

impl PreparedActorRoute {
    pub(crate) fn words(self, active: [u32; 3]) -> [u32; 3] {
        if self.arts_eligible && f32::from_bits(active[0]) > 0.0 {
            active
        } else {
            [0, u32::MAX, (-1.0f32).to_bits()]
        }
    }

    pub(crate) fn flags(self, flags: u32, arts_word: u32) -> u32 {
        const FOLLOW_UP: u32 = 1 << 25;
        let follow_up = self.follow_up_eligible && f32::from_bits(arts_word) <= 0.0;
        (flags & !FOLLOW_UP) | if follow_up { FOLLOW_UP } else { 0 }
    }
}

impl ActiveArts {
    /// Six is a representative of the capped binary32 result. At five keys
    /// PC202's multiply-before-min can produce the adjacent lower float.
    pub fn saturated() -> Self {
        Self { distinct_keys: 6, owner_additive_bits: 0, ki_phase: KiPhase::Charging }
    }

    /// The percent in PC202 definition 444, parameter 4. Multiply on the owner
    /// side before applying the result to the candidate's other Ki modifiers.
    pub(crate) fn owner_ki_factor(self, owner_factor: f32) -> f32 {
        owner_factor * (70.0f32 * 0.01)
    }
}

#[cfg(test)]
mod tests {
    use super::PreparedActorRoute;

    #[test]
    fn prepared_actor_arts_and_follow_up_are_independent() {
        let active = [0.24f32.to_bits(), 6, 1.0f32.to_bits()];
        let absent = [0, u32::MAX, (-1.0f32).to_bits()];
        let both = PreparedActorRoute {
            arts_eligible: true,
            follow_up_eligible: true,
        };
        assert_eq!(both.words(active), active);
        assert_eq!(both.words([0; 3]), absent);
        assert_eq!(both.flags(0x0600_00C2, active[0]), 0x0400_00C2);
        assert_eq!(both.flags(0x0400_00C2, absent[0]), 0x0600_00C2);
        assert_eq!(both.flags(0x0600_00C2, f32::NAN.to_bits()), 0x0400_00C2);

        let suppressed = PreparedActorRoute {
            arts_eligible: true,
            follow_up_eligible: false,
        };
        assert_eq!(suppressed.words(active), active);
        assert_eq!(suppressed.flags(0x0600_00C2, absent[0]), 0x0400_00C2);

        let follow_up_only = PreparedActorRoute {
            arts_eligible: false,
            follow_up_eligible: true,
        };
        assert_eq!(follow_up_only.words(active), absent);
        assert_eq!(follow_up_only.flags(0x0400_00C2, absent[0]), 0x0600_00C2);
    }
}
