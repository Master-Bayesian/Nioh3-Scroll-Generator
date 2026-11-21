//! The two inline SOURCE maps consumed by the native group lookup (921BEF).
use crate::{context::EncodedContext, pipeline::Entry};

const OFFSETS: [u64; 2] = [0x35d0, 0x4728];
const CAPACITY: usize = 256;
const ROW_BYTES: usize = 12;
const COUNT: usize = CAPACITY * ROW_BYTES;
const SIZE: usize = COUNT + 4;

pub(super) struct CandidateSourceMaps {
    address: u64,
    pub(super) actor_category: u64,
    pub(super) inventory_record: Option<u64>,
    initial: [[u8; SIZE]; 2],
}

impl CandidateSourceMaps {
    pub(super) fn new(
        context: &EncodedContext,
        stat: u64,
        main: u64,
        address: u64,
        actor_category: u64,
        inventory_record: Option<u64>,
    ) -> Result<Self, String> {
        let link = stat.checked_add(0x7d8).ok_or("STAT link overflow")?;
        if u64::from_le_bytes(context.read_known(link).map_err(|e| e.to_string())?) != address {
            return Err("attack and role identify different selected SOURCE objects".into());
        }
        let mut initial = [[0; SIZE]; 2];
        for (buffer, offset) in initial.iter_mut().zip(OFFSETS) {
            let start = address
                .checked_add(offset)
                .ok_or("SOURCE address overflow")?;
            *buffer = context.read_known(start).map_err(|e| e.to_string())?;
        }
        let pointer = |base: u64, offset: u64| -> Result<u64, String> {
            let field = base.checked_add(offset).ok_or("pointer field overflow")?;
            Ok(u64::from_le_bytes(
                context.read_known(field).map_err(|e| e.to_string())?,
            ))
        };
        let actor = pointer(main, 0x198)?;
        let controller = pointer(pointer(actor, 0x3b0)?, 8)?;
        if controller.checked_add(0xa68) != Some(actor_category) {
            return Err("candidate category field differs from the actor controller".into());
        }
        if let Some(record) = inventory_record {
            let owner = pointer(actor, 0x3a0)?;
            let inv = owner.checked_add(0x590).ok_or("owner inventory overflow")?;
            let bank_field = inv.checked_add(0x1ee0).ok_or("bank selector overflow")?;
            let bank =
                u32::from_le_bytes(context.read_known(bank_field).map_err(|e| e.to_string())?);
            if bank > 1 {
                return Err("invalid inventory bank".into());
            }
            let base = inv
                .checked_add(u64::from(bank) * 0xf70)
                .ok_or("inventory bank overflow")?;
            let slot_field = base.checked_add(0xf68).ok_or("slot selector overflow")?;
            let [selector] = context.read_known(slot_field).map_err(|e| e.to_string())?;
            if base.checked_add(u64::from(selector == 0) * 0xe8) != Some(record) {
                return Err("candidate inventory and native held record differ".into());
            }
        }
        Ok(Self {
            address,
            actor_category,
            inventory_record,
            initial,
        })
    }

    pub(super) fn append(
        &self,
        maps: &[Vec<Entry>; 2],
        writes: &mut Vec<(u64, u32)>,
    ) -> Result<(), &'static str> {
        for ((entries, initial), offset) in maps.iter().zip(&self.initial).zip(OFFSETS) {
            if entries.len() > CAPACITY {
                return Err("candidate SOURCE exceeds its native map capacity");
            }
            let start = self.address + offset;
            let word = |at| {
                u32::from_le_bytes([
                    initial[at],
                    initial[at + 1],
                    initial[at + 2],
                    initial[at + 3],
                ])
            };
            let mut changed = |at, before, after| {
                if before != after {
                    writes.push((start + at as u64, after));
                }
            };
            for (i, entry) in entries.iter().enumerate() {
                let at = i * ROW_BYTES;
                let key = word(at);
                let dup = word(at + 8);
                changed(at, key, key & 0xffff0000 | u32::from(entry.key));
                changed(at + 4, word(at + 4), entry.base);
                changed(at + 8, dup, dup & 0xffff0000 | u32::from(entry.dup));
            }
            changed(COUNT, word(COUNT), entries.len() as u32);
        }
        Ok(())
    }
}
