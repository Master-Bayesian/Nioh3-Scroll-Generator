//! PC202 static damage descriptors. A row selector is local to one exact
//! resource/action/parameter identity and does not imply a landed contact.
use serde::Deserialize;
use sha2::{Digest, Sha256};

const BANK: &[u8] = include_bytes!("pc202_damage_rows.bin");
const ROW_BYTES: usize = 192;

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct ActionRow {
    pub resource: String,
    pub action: u16,
    pub parameter_sha256: String,
    pub row: u32,
    pub row_sha256: String,
}

impl ActionRow {
    pub fn resolve(&self) -> Result<ResolvedActionRow, &'static str> {
        let invalid = || "invalid PC202 attack parameter index";
        let (header, payload) = BANK.split_first_chunk::<16>().ok_or_else(invalid)?;
        let word = |bytes: &[u8]| bytes.try_into().ok().map(u32::from_le_bytes);
        let mappings = word(&header[8..12]).ok_or_else(invalid)? as usize;
        let bodies = word(&header[12..16]).ok_or_else(invalid)? as usize;
        let (index, data) = payload
            .split_at_checked(mappings * 36)
            .ok_or_else(invalid)?;
        let (rows, tail) = data.as_chunks::<ROW_BYTES>();
        if &header[..8] != b"N3AAD001" || !tail.is_empty() || rows.len() != bodies {
            return Err(invalid());
        }
        let mut parameter = [0u8; 32];
        if self.parameter_sha256.len() != 64
            || !self
                .parameter_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("invalid attack parameter digest");
        }
        for (i, byte) in parameter.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&self.parameter_sha256[2 * i..2 * i + 2], 16)
                .map_err(|_| "invalid attack parameter digest")?;
        }
        let mut hasher = Sha256::new();
        hasher.update(self.resource.as_bytes());
        hasher.update([0]);
        hasher.update(self.action.to_le_bytes());
        hasher.update(self.row.to_le_bytes());
        hasher.update(parameter);
        let key = hasher.finalize();
        let (entries, _) = index.as_chunks::<36>();
        let position = entries
            .binary_search_by(|entry| entry[..32].cmp(&key))
            .map_err(|_| "unbound resource/action/parameter/row combination")?;
        let slot = word(&entries[position][32..]).ok_or_else(invalid)? as usize;
        let row = rows.get(slot).ok_or_else(invalid)?;
        if format!("{:x}", Sha256::digest(row)) != self.row_sha256 {
            return Err("selected damage row digest differs from its descriptor");
        }
        Ok(ResolvedActionRow {
            key: key.into(),
            bytes: row,
        })
    }
}

pub struct ResolvedActionRow {
    pub key: [u8; 32],
    pub bytes: &'static [u8; ROW_BYTES],
}

/// Native producers reuse the packet's descriptor object across contacts. The
/// first row initializes that object; a contact writes the words that can vary.
/// Emit the complete set of varying columns for *each* contact, including
/// returns to the first row. The previous contact never owns the current row.
pub struct DamageRowProgram {
    pub initial: &'static [u8; ROW_BYTES],
    pub offsets: Vec<u8>,
    pub words: Vec<Vec<u32>>,
}

impl DamageRowProgram {
    pub fn compile(rows: &[ResolvedActionRow]) -> Result<Self, &'static str> {
        let initial = rows
            .first()
            .ok_or("damage row program requires a contact")?
            .bytes;
        let mut offsets = Vec::new();
        let mut words = vec![Vec::new(); rows.len()];
        for (i, first) in initial.as_chunks::<4>().0.iter().enumerate() {
            let at = i * 4;
            if rows.iter().any(|row| row.bytes[at..at + 4] != first[..]) {
                offsets.push(at as u8);
                for (values, row) in words.iter_mut().zip(rows) {
                    values.push(u32::from_le_bytes(row.bytes.as_chunks::<4>().0[i]));
                }
            }
        }
        Ok(Self {
            initial,
            offsets,
            words,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_variant_restores_direct_and_projectile_rows_in_order() -> Result<(), &'static str> {
        let direct = ActionRow {
            resource: "0x600000184".into(),
            action: 25358,
            row: 0,
            parameter_sha256: "ddcdfd01b4ec43a6a63d224b31b6cdfbdc8a2925c6a834394872511f683c1780"
                .into(),
            row_sha256: "8412ecc647d400d17d19e03bcab01cae65f6a67e629fe924dd2c95ab62ad65c1".into(),
        };
        let mut projectile = ActionRow {
            resource: direct.resource.clone(),
            action: 25517,
            row: 1,
            parameter_sha256: "2881ebaad6c7b2bdad1ae619be48799d2b2828cb4a14abdd22c6458b6cbc78ef"
                .into(),
            row_sha256: "3f92c61d322eea649e2d097fd5f87a8565f02c2d6cfb975631cfd10f623edfe8".into(),
        };
        let inputs = [direct.resolve()?, projectile.resolve()?, direct.resolve()?];
        let program = DamageRowProgram::compile(&inputs)?;
        let mut scratch = *program.initial;
        for (values, expected) in program.words.iter().zip(&inputs) {
            for (&offset, &value) in program.offsets.iter().zip(values) {
                let at = usize::from(offset);
                scratch[at..at + 4].copy_from_slice(&value.to_le_bytes());
            }
            assert_eq!(&scratch, expected.bytes);
        }
        assert_ne!(inputs[0].bytes, inputs[1].bytes);
        projectile.row = 2;
        assert!(projectile.resolve().is_err());
        projectile.resource = "0x60000010f".into();
        assert!(projectile.resolve().is_err());
        Ok(())
    }
}
