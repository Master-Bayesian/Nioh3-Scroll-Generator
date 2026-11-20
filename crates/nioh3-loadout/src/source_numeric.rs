//! PC202 guardian/core numeric additions; separate from ordinary source maps.
//! Item caps and parameter bodies are explicit input contracts, not output fits.
use super::{cvtt64, u16_at, u32_at, Character, Sections, Tables, R};
use sha2::{Digest, Sha256};

pub struct Inputs<'a> {
    pub guardian_records: &'a [u8],
    pub active_guardian: u8,
    pub guardian_numbers_disabled: u8,
    pub core_records: &'a [u8],
}
pub struct Parameters {
    guardian: Vec<u8>,
    rank: Vec<u8>,
    passive: Vec<u8>,
    digest: String,
}
#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Part {
    Guardian {
        key: u32,
        rank: u16,
        attack: u32,
        defense: u32,
    },
    Core {
        slot: usize,
        key: u16,
        normalized_level: u32,
        normalized_plus: u32,
        attack: u32,
        defense: u32,
    },
    MissingDefinition {
        role: &'static str,
        key: u32,
    },
    DisabledGuardian,
}
#[derive(Debug, serde::Serialize)]
pub struct Seed {
    pub attack_gain: u32,
    pub defense: u32,
    pub parts: Vec<Part>,
    pub parameters_sha256: String,
}
impl Parameters {
    pub fn load(raw: &[u8]) -> R<Self> {
        let sections = Sections::parse(raw)?;
        if sections.get("version")?.as_slice() != 1u32.to_le_bytes() {
            return Err("unsupported source numeric parameter layout");
        }
        let guardian = sections.get("guardian3f0")?.clone();
        let rank = sections.get("guardian408")?.clone();
        let passive = sections.get("passive118")?.clone();
        for (rows, stride) in [(&guardian, 256usize), (&rank, 64), (&passive, 84)] {
            if rows.len() < 8
                || (rows.len() - 8) / stride != u32_at(rows, 4) as usize
                || (rows.len() - 8) % stride != 0
            {
                return Err("source numeric parameter extent differs");
            }
        }
        for (rows, stride, offset, width) in [(&guardian, 256, 0x24, 4), (&passive, 84, 0xc, 2)] {
            let mut keys = std::collections::BTreeSet::new();
            for row in rows[8..].chunks_exact(stride) {
                let key = if width == 4 {
                    u32_at(row, offset)
                } else {
                    u32::from(u16_at(row, offset))
                };
                if !keys.insert(key) {
                    return Err("source numeric parameter key is duplicated");
                }
            }
        }
        Ok(Self {
            guardian,
            rank,
            passive,
            digest: format!("{:x}", Sha256::digest(raw)),
        })
    }
    pub fn derive(&self, tables: &Tables, character: &Character, source: &[u8]) -> R<Seed> {
        if source.len() != 0x6000 {
            return Err("complete selected SOURCE required for numeric inputs");
        }
        self.derive_inputs(
            tables,
            character,
            Inputs {
                guardian_records: &source[0x58..0x68],
                active_guardian: source[0x70],
                guardian_numbers_disabled: source[0x71],
                core_records: &source[0x80..0x250],
            },
        )
    }
    pub fn derive_inputs(
        &self,
        tables: &Tables,
        character: &Character,
        inputs: Inputs<'_>,
    ) -> R<Seed> {
        if inputs.guardian_records.len() != 16 || inputs.core_records.len() != 2 * 0xe8 {
            return Err("source numeric logical input extent differs");
        }
        let mut result = Seed {
            attack_gain: 0,
            defense: 0,
            parts: Vec::new(),
            parameters_sha256: self.digest.clone(),
        };
        if inputs.guardian_numbers_disabled != 0 {
            result.parts.push(Part::DisabledGuardian);
        } else {
            let selected = usize::from(inputs.active_guardian);
            if selected >= 2 {
                return Err("guardian selector outside native two-record domain");
            }
            let key = u32_at(inputs.guardian_records, 8 * selected);
            let level = u16_at(inputs.guardian_records, 4 + 8 * selected);
            match self.guardian[8..]
                .as_chunks::<256>()
                .0
                .iter()
                .find(|row| u32_at(*row, 0x24) == key)
                .filter(|row| u32_at(*row, 0x24) != 1)
            {
                Some(row) => {
                    let rank = self.rank[8..].as_chunks::<64>().0.get(usize::from(level));
                    let value = |base, slope, column| {
                        let base = u32_at(row, base);
                        match rank {
                            None => base,
                            Some(rank) => {
                                let product = u32_at(row, slope)
                                    .wrapping_mul(u32::from(u16_at(rank, column)));
                                base.wrapping_add(cvtt64(product as f32 * tables.c_4688) as u32)
                            }
                        }
                    };
                    let attack = value(0x5c, 0x60, 0x16);
                    let defense = value(0x64, 0x68, 0x18);
                    result.attack_gain = attack;
                    result.defense = defense;
                    result.parts.push(Part::Guardian {
                        key,
                        rank: level,
                        attack,
                        defense,
                    });
                }
                None => result.parts.push(Part::MissingDefinition {
                    role: "guardian",
                    key,
                }),
            }
        }
        for slot in 0..2 {
            let item = &inputs.core_records[slot * 0xe8..(slot + 1) * 0xe8];
            let key = u16_at(item, 0);
            let Some(row) = tables.items.row(u32::from(key)).and_then(|definition| {
                self.passive[8..]
                    .as_chunks::<84>()
                    .0
                    .iter()
                    .find(|row| u16_at(*row, 0xc) == u16_at(definition, 0x180))
            }) else {
                result.parts.push(Part::MissingDefinition {
                    role: "core",
                    key: u32::from(key),
                });
                continue;
            };
            // Original2FA2F0 normalization is shared with the equipment path.
            let effective = tables.effective(character, item, true)?;
            let level = effective.level.min(190);
            let plus = effective.plus.min(character.plus_cap);
            if plus > 500 || level.max(plus) as usize >= tables.base70.len() / 148 {
                return Err("source core level outside admitted level-table domain");
            }
            let value = |base, scale, column| {
                let combined = tables
                    .field70(level, column)
                    .wrapping_add(tables.field70(plus, column + 2));
                let product = combined.wrapping_mul(u32::from(u16_at(row, scale))) as i32;
                (product / 1000).wrapping_add(i32::from(u16_at(row, base))) as u32
            };
            let attack = value(0x34, 0x36, 0x36) & 0xffff;
            let defense = value(0x38, 0x3a, 0x3a);
            // Original55445A supplies EDX=0 to818200, so the bonus is zero;
            // the subsequent add is16-bit. Never feed candidate attributes here.
            result.attack_gain = result.attack_gain.wrapping_add(attack);
            result.defense = result.defense.wrapping_add(defense);
            result.parts.push(Part::Core {
                slot,
                key,
                normalized_level: level,
                normalized_plus: plus,
                attack,
                defense,
            });
        }
        Ok(result)
    }
}
