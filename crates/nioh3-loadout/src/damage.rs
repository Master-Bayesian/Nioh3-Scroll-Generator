//! Independent native output channels and their ordered move accumulation.

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct DamageWords {
    pub total: u32,
    pub physical: u32,
    pub elemental: u32,
    pub reserved: u32,
}
const _: () = assert!(std::mem::size_of::<DamageWords>() == 16);

#[derive(Clone, Debug, serde::Serialize)]
pub struct ContactDamage {
    pub contact: usize,
    pub template: usize,
    pub total: f32,
    pub physical: f32,
    pub elemental: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailUnavailable {
    MissingComponentCapability,
    UnsupportedRecordedScenario,
    EvaluationFailed,
    NonFiniteOutput,
    ScoreMismatch,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DamageDetails {
    Available {
        total: f32,
        physical: f32,
        elemental: f32,
        contacts: Vec<ContactDamage>,
    },
    Unavailable {
        reason: DetailUnavailable,
    },
}
impl DamageDetails {
    pub fn unavailable(reason: DetailUnavailable) -> Self {
        Self::Unavailable { reason }
    }
    pub fn verify_total(self, expected: u32) -> Self {
        match self.total_bits() {
            Some(actual) if actual != expected => {
                Self::unavailable(DetailUnavailable::ScoreMismatch)
            }
            _ => self,
        }
    }
    pub fn total_bits(&self) -> Option<u32> {
        match self {
            Self::Available { total, .. } => Some(total.to_bits()),
            Self::Unavailable { .. } => None,
        }
    }
}

pub(crate) struct DamageAccumulator {
    total: f32,
    physical: f32,
    elemental: f32,
    contacts: Vec<ContactDamage>,
}
impl DamageAccumulator {
    pub fn new() -> Self {
        Self {
            total: 0.0,
            physical: 0.0,
            elemental: 0.0,
            contacts: Vec::new(),
        }
    }
    pub fn push(&mut self, template: usize, words: DamageWords) -> Result<(), DetailUnavailable> {
        let [total, physical, elemental] =
            [words.total, words.physical, words.elemental].map(f32::from_bits);
        if words.reserved != 0 || [total, physical, elemental].iter().any(|v| !v.is_finite()) {
            return Err(DetailUnavailable::NonFiniteOutput);
        }
        // Keep all three sums independent and preserve contact order. Adding
        // channel totals at the end can have different binary32 rounding.
        self.total += total;
        self.physical += physical;
        self.elemental += elemental;
        if [self.total, self.physical, self.elemental]
            .iter()
            .any(|v| !v.is_finite())
        {
            return Err(DetailUnavailable::NonFiniteOutput);
        }
        self.contacts.push(ContactDamage {
            contact: self.contacts.len() + 1,
            template: template + 1,
            total,
            physical,
            elemental,
        });
        Ok(())
    }
    pub fn finish(self) -> DamageDetails {
        DamageDetails::Available {
            total: self.total,
            physical: self.physical,
            elemental: self.elemental,
            contacts: self.contacts,
        }
    }
}
