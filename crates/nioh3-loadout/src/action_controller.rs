//! Resource-local controller records, including the native first index and ordered row references.
use super::{ActionRow, BANK, ROW_BYTES};
use sha2::{Digest, Sha256};

const STATES: &[u8] = include_bytes!("pc202_action_states.bin");

pub struct ActionController {
    pub parameter: &'static [u8; 336],
    pub first_index: u16,
    pub rows: Vec<&'static [u8; ROW_BYTES]>,
}

impl ActionRow {
    pub fn controller(&self) -> Result<ActionController, &'static str> {
        ActionController::resolve(&self.resource, self.action, &self.parameter_sha256)
    }
}

impl ActionController {
    pub fn resolve(
        resource: &str,
        action: u16,
        parameter_sha256: &str,
    ) -> Result<ActionController, &'static str> {
        let invalid = "invalid PC202 action controller bank";
        let (head, data) = STATES.split_first_chunk::<20>().ok_or(invalid)?;
        let word = |bytes: &[u8]| -> Result<usize, &'static str> {
            Ok(u32::from_le_bytes(bytes.try_into().map_err(|_| invalid)?) as usize)
        };
        if &head[..8] != b"N3ACT001" {
            return Err(invalid);
        }
        let (entries, data) = data
            .split_at_checked(word(&head[8..12])?.checked_mul(44).ok_or(invalid)?)
            .ok_or(invalid)?;
        let (parameters, refs) = data
            .split_at_checked(word(&head[12..16])?.checked_mul(336).ok_or(invalid)?)
            .ok_or(invalid)?;
        let (refs, extra) = refs.as_chunks::<4>();
        if !extra.is_empty() || refs.len() != word(&head[16..20])? {
            return Err(invalid);
        }
        let digest = super::parameter_digest(parameter_sha256)?;
        let mut id = Sha256::new();
        id.update(resource.as_bytes());
        id.update([0]);
        id.update(action.to_le_bytes());
        id.update(digest);
        let key = id.finalize();
        let entries = entries.as_chunks::<44>().0;
        let i = entries
            .binary_search_by(|entry| entry[..32].cmp(&key))
            .map_err(|_| "unbound action controller")?;
        let entry = &entries[i];
        let parameter = parameters
            .as_chunks::<336>()
            .0
            .get(word(&entry[32..36])?)
            .ok_or(invalid)?;
        if Sha256::digest(parameter)[..] != digest {
            return Err("action controller parameter digest differs");
        }
        let first = word(&entry[36..40])?;
        let count = u16::from_le_bytes([entry[40], entry[41]]);
        let first_index = u16::from_le_bytes([entry[42], entry[43]]);
        let slots = refs
            .get(first..first.checked_add(usize::from(count)).ok_or(invalid)?)
            .ok_or(invalid)?;
        let index_count = word(BANK.get(8..12).ok_or(invalid)?)?;
        let start = index_count
            .checked_mul(36)
            .and_then(|v| v.checked_add(16))
            .ok_or(invalid)?;
        let (bodies, tail) = BANK.get(start..).ok_or(invalid)?.as_chunks::<ROW_BYTES>();
        if !tail.is_empty() {
            return Err(invalid);
        }
        let rows = slots
            .iter()
            .map(|v| bodies.get(u32::from_le_bytes(*v) as usize).ok_or(invalid))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ActionController {
            parameter,
            first_index,
            rows,
        })
    }
}
