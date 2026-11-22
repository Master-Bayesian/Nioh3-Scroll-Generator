//! Inputs from the same ordinary read as the selected Samurai character.
use crate::error::HostError;
use nioh3_loadout::arts_proficiency::{ActiveArts, KiPhase, Start};
use nioh3_loadout::water_sword::Role;
use nioh3_runtime::{combat_context::CombatSnapshot, InventoryMemory};
use std::collections::BTreeSet;

struct Reader<'a>(&'a dyn InventoryMemory);
impl Reader<'_> {
    fn bytes<const N: usize>(&self, base: u64, delta: u64) -> Result<[u8; N], HostError> {
        let address = base
            .checked_add(delta)
            .filter(|_| base != 0)
            .ok_or_else(|| HostError::rejected("Missing held action input pointer"))?;
        self.0
            .read(address, N)
            .map_err(HostError::from_runtime)?
            .try_into()
            .map_err(|_| HostError::rejected("Incomplete held action input"))
    }
    fn ptr(&self, at: u64, off: u64) -> Result<u64, HostError> {
        Ok(u64::from_le_bytes(self.bytes(at, off)?))
    }
    fn word(&self, at: u64, off: u64) -> Result<u32, HostError> {
        Ok(u32::from_le_bytes(self.bytes(at, off)?))
    }
    fn effect(&self, actor: u64, owner: u64) -> Result<Option<ActiveArts>, HostError> {
        if self.bytes::<1>(owner, 0x3368)? != [0] {
            return Ok(None);
        }
        for off in [0x3328, 0x3308] {
            let sentinel = self.ptr(owner, off)?;
            let mut node = self.ptr(sentinel, 0)?;
            let mut seen = BTreeSet::new();
            while node != sentinel {
                if !seen.insert(node) || seen.len() > 1024 {
                    return Err(HostError::rejected("Inconsistent effect list"));
                }
                let object = self.ptr(node, 0x10)?;
                if object != 0 && self.bytes::<1>(object, 0x14)?[0] & 0x20 != 0 {
                    let duration = f32::from_bits(self.word(object, 0x38)?);
                    let remaining = f32::from_bits(self.word(object, 0x34)?);
                    if duration < 0.0 || remaining > 0.0 {
                        let getter = self.ptr(self.ptr(object, 0)?, 0x10)?;
                        if getter.checked_sub(self.0.module_base()) != Some(0x3faa7c) {
                            return Err(HostError::rejected(
                                "Unresolved active effect type getter",
                            ));
                        }
                        // PC202 native hash key 7011 resolves definition row 27 (row ID 444).
                        // Its type is 36D8 and both additional validity flags are clear.
                        if self.word(object, 0x1c)? == 0x7011 {
                            if self.ptr(object, 8)? != actor {
                                return Err(HostError::rejected(
                                    "Active effect belongs to a different actor",
                                ));
                            }
                            return Ok(Some(ActiveArts {
                                distinct_keys: self.ptr(object, 0xa0)?,
                                owner_additive_bits: self.word(owner, 0x2df4)?,
                                ki_phase: KiPhase::from_object(self.word(object, 0xb0)?),
                            }));
                        }
                    }
                }
                node = self.ptr(node, 0)?;
            }
        }
        Ok(None)
    }
}
pub(super) fn read(
    memory: &dyn InventoryMemory,
    snap: &CombatSnapshot,
    start: Start,
) -> Result<Role, HostError> {
    let input = &snap.inputs;
    if (input.mode, input.held_weapon_slot, input.style) != (0, 0, 0)
        || input.maximum_health == 0
        || input.current_health != input.maximum_health
    {
        return Err(HostError::rejected(
            "Water Sword currently requires full health, Samurai high stance and the primary slot",
        ));
    }
    let learned = |key| {
        input
            .configured_skills
            .iter()
            .any(|s| s.key == key && s.enabled != 0 && s.rank != 0)
    };
    if !learned(0x98cf) {
        return Err(HostError::rejected(
            "Water Sword is not enabled for this character",
        ));
    }
    let r = Reader(memory);
    let actor = snap.roots.player;
    let owner = snap.roots.owner;
    if r.ptr(actor, 0x3a0)? != owner {
        return Err(HostError::rejected("Held action owner changed"));
    }
    let descriptor = r.ptr(actor, 0xe8)?;
    let control = r.ptr(actor, 0x3b0)?;
    let mask = if control == 0 {
        0
    } else {
        r.ptr(control, 0x40)?
    };
    let cost = f32::from_bits(r.word(owner, 0x2e80)?) * f32::from_bits(r.word(owner, 0x2e84)?);
    let effect = match start {
        Start::Current => r
            .effect(actor, owner)
            .map_err(|e| HostError::rejected(format!("LOADOUT_MARTIAL_EFFECT_UNOBSERVED: {e}")))?,
        Start::Untriggered => None,
        Start::TheoreticalPeak => learned(0x6842).then(ActiveArts::saturated),
        Start::FullGaugeActivation => {
            if !learned(0x6842) {
                return Err(HostError::rejected(
                    "Full-gauge activation requires Arts Proficiency",
                ));
            }
            // Start at the accepted grant, with no pre-existing transient sources.
            // 1DA contributes to the owner additive only after its own style-shift event.
            Some(ActiveArts {
                distinct_keys: 0,
                owner_additive_bits: 0,
                ki_phase: KiPhase::Ready,
            })
        }
    };
    Ok(Role {
        upgraded: learned(0x2e74),
        agility_adjusted: descriptor != 0 && r.word(descriptor, 0x68)? >> 31 != 0,
        suppress_general: mask != 0 && r.bytes::<1>(mask, 0)?[0] & 4 != 0,
        high_stance_cost: cost.to_bits(),
        effect,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nioh3_runtime::FixtureMemory;
    use serde_json::json;
    #[test]
    fn current_object_uses_native_lookup_key_and_ordered_membership() -> Result<(), HostError> {
        let owner = 0x2000u64;
        let actor = 0x1234u64;
        let mut bytes = vec![0; 0x3400];
        let put = |b: &mut [u8], i: usize, n: u64| b[i..i + 8].copy_from_slice(&n.to_le_bytes());
        for (head, sent, node, obj, key, counter) in [
            (0x3328, 0x100, 0x180, 0x200, 444u32, 9u64),
            (0x3308, 0x400, 0x480, 0x500, 0x7011, 2),
        ] {
            put(&mut bytes, head, owner + sent as u64);
            put(&mut bytes, sent, owner + node as u64);
            put(&mut bytes, node, owner + sent as u64);
            put(&mut bytes, node + 0x10, owner + obj as u64);
            put(&mut bytes, obj, owner + 0x900);
            put(&mut bytes, obj + 8, actor);
            bytes[obj + 0x14] = 0x20;
            bytes[obj + 0x1c..obj + 0x20].copy_from_slice(&key.to_le_bytes());
            bytes[obj + 0x38..obj + 0x3c].copy_from_slice(&(-1.0f32).to_le_bytes());
            put(&mut bytes, obj + 0xa0, counter);
        }
        put(&mut bytes, 0x910, 4096 + 0x3faa7c);
        bytes[0x2df4..0x2df8].copy_from_slice(&0.5f32.to_le_bytes());
        let query = |b: &[u8]| {
            let hex = b.iter().map(|v| format!("{v:02x}")).collect::<String>();
            let memory =
                FixtureMemory::from_json(&json!({"pid":1,"creation_filetime":2,"module_base":4096,
                "regions":[{"address":owner,"bytes":hex}]}))
                .map_err(HostError::from_runtime)?;
            Reader(&memory).effect(actor, owner)
        };
        let effect = ActiveArts {
            distinct_keys: 2,
            owner_additive_bits: 0.5f32.to_bits(),
            ki_phase: KiPhase::Ready,
        };
        assert_eq!(query(&bytes)?, Some(effect));
        bytes[0x5b0..0x5b4].copy_from_slice(&2u32.to_le_bytes());
        assert_eq!(
            query(&bytes)?,
            Some(ActiveArts {
                ki_phase: KiPhase::Charging,
                ..effect
            })
        );
        bytes[0x538..0x53c].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(query(&bytes)?, None);
        put(&mut bytes, 0x910, 4096 + 0xbeef);
        assert!(query(&bytes).is_err());
        bytes[0x3368] = 1;
        assert_eq!(query(&bytes)?, None);
        Ok(())
    }
}
