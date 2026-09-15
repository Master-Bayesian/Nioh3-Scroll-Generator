//! Byte-exact ports of the shipped trampoline builders.
//!
//! `build_override_trampoline` and `build_challenge_trampoline` produce the
//! position-independent x64 code the game executes from a temporary hook, so
//! `tests/migration/test_runtime_mutation_parity.py` compares them byte-for-byte
//! with the shipped Python builders. Nothing here touches a process.

use crate::error::RuntimeError;

/// Maximum code size of one trampoline (`REMOTE_ALLOCATION_SIZE - 8`).
pub const TRAMPOLINE_CAPACITY: usize = 0x1000 - 8;

/// Maximum number of enemy groups one auxiliary profile may request.
pub const MAX_ENEMY_GROUPS: usize = 8;

/// One resolved enemy group: the display lookup key plus the native role the
/// descriptor must carry beside it.
///
/// The role is resolved by the caller from the shipped auxiliary candidate
/// table (`_enemy_role_by_lookup_key`), which keeps this crate free of product
/// data files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnemyGroup {
    pub lookup_key: u32,
    pub role: u32,
}

/// One non-persistent descriptor override keyed by displayed Seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverrideProfile {
    pub seed: u32,
    pub enemy_groups: Vec<EnemyGroup>,
    pub special_rule_keys: Option<[u16; 3]>,
    pub terrain_value: Option<u8>,
}

impl OverrideProfile {
    /// `RuntimeAuxiliaryOverrideProfile.__post_init__`.
    pub fn validate(&self) -> Result<(), RuntimeError> {
        if self.enemy_groups.len() > MAX_ENEMY_GROUPS {
            return Err(RuntimeError::InvalidOverrideProfile {
                detail: "at most eight enemy groups are supported".to_string(),
            });
        }
        if self.enemy_groups.is_empty()
            && self.special_rule_keys.is_none()
            && self.terrain_value.is_none()
        {
            return Err(RuntimeError::InvalidOverrideProfile {
                detail: "an override profile must change at least one field".to_string(),
            });
        }
        Ok(())
    }
}

/// `build_relative_jump`: a five-byte `E9 rel32` from source to target.
pub fn build_relative_jump(
    source_address: u64,
    target_address: u64,
) -> Result<Vec<u8>, RuntimeError> {
    let delta = target_address as i128 - (source_address as i128 + 5);
    if !(-(1i128 << 31)..(1i128 << 31)).contains(&delta) {
        return Err(RuntimeError::TrampolineOutOfRange {
            source: source_address,
            target: target_address,
        });
    }
    let mut code = Vec::with_capacity(5);
    code.push(0xE9);
    code.extend_from_slice(&(delta as i32).to_le_bytes());
    Ok(code)
}

/// `build_override_trampoline`.
pub fn build_override_trampoline(
    profile: &OverrideProfile,
    return_address: u64,
    counter_address: Option<u64>,
    original_instruction: &[u8],
) -> Result<Vec<u8>, RuntimeError> {
    profile.validate()?;
    if original_instruction.is_empty() {
        return Err(RuntimeError::InvalidOverrideProfile {
            detail: "original_instruction must not be empty".to_string(),
        });
    }
    let mut builder = CodeBuilder::new();
    // pushfq; push rax, rcx, rdx, r8, r9, r10, r11
    builder.emit(&[
        0x9C, 0x50, 0x51, 0x52, 0x41, 0x50, 0x41, 0x51, 0x41, 0x52, 0x41, 0x53,
    ]);
    builder.emit(&[0x41, 0x81, 0xFC]);
    builder.emit(&profile.seed.to_le_bytes());
    builder.branch(&[0x0F, 0x85], "done"); // jne done

    if !profile.enemy_groups.is_empty() {
        let required_bytes = (profile.enemy_groups.len() * 0x28) as u32;
        builder.emit(&[0x48, 0x8B, 0x45, 0x00]);
        builder.emit(&[0x48, 0x85, 0xC0]);
        builder.branch(&[0x0F, 0x84], "done"); // je done
        builder.emit(&[0x48, 0x8B, 0x4D, 0x08]);
        builder.emit(&[0x48, 0x39, 0xC1]);
        builder.branch(&[0x0F, 0x82], "done"); // jb done
        builder.emit(&[0x48, 0x8B, 0x55, 0x10]);
        builder.emit(&[0x48, 0x39, 0xCA]);
        builder.branch(&[0x0F, 0x82], "done");
        builder.emit(&[0x49, 0x89, 0xC8, 0x49, 0x29, 0xC0]);
        builder.emit(&[0x49, 0x81, 0xF8]);
        builder.emit(&required_bytes.to_le_bytes());
        builder.branch(&[0x0F, 0x82], "done");

        // Validate every native inner vector before mutating any descriptor.
        for index in 0..profile.enemy_groups.len() {
            let group_offset = (index * 0x28) as u32;
            builder.emit(&[0x4C, 0x8B, 0x80]);
            builder.emit(&group_offset.to_le_bytes());
            builder.emit(&[0x4D, 0x85, 0xC0]);
            builder.branch(&[0x0F, 0x84], "done");
            builder.emit(&[0x4C, 0x8B, 0x88]);
            builder.emit(&(group_offset + 0x08).to_le_bytes());
            builder.emit(&[0x4D, 0x39, 0xC1]);
            builder.branch(&[0x0F, 0x82], "done");
            builder.emit(&[0x4C, 0x8B, 0x90]);
            builder.emit(&(group_offset + 0x10).to_le_bytes());
            builder.emit(&[0x4D, 0x39, 0xCA]);
            builder.branch(&[0x0F, 0x82], "done");
            builder.emit(&[0x4D, 0x8D, 0x58, 0x14]);
            builder.emit(&[0x4D, 0x39, 0xD9]);
            builder.branch(&[0x0F, 0x82], "done");
        }

        for (index, group) in profile.enemy_groups.iter().enumerate() {
            let group_offset = (index * 0x28) as u32;
            builder.emit(&[0x4C, 0x8B, 0x80]);
            builder.emit(&group_offset.to_le_bytes());
            builder.emit(&[0x41, 0xC7, 0x40, 0x04]);
            builder.emit(&group.lookup_key.to_le_bytes());
            // Descriptor role beside the lookup key: updating only the key
            // leaves an internally inconsistent entry that the challenge
            // consumer may discard even though the detail UI shows the name.
            builder.emit(&[0x41, 0xC7, 0x40, 0x08]);
            builder.emit(&group.role.to_le_bytes());
            builder.emit(&[0x4D, 0x8D, 0x48, 0x14]);
            builder.emit(&[0x4C, 0x89, 0x88]);
            builder.emit(&(group_offset + 0x08).to_le_bytes());
        }
        builder.emit(&[0x4C, 0x8D, 0x80]);
        builder.emit(&required_bytes.to_le_bytes());
        builder.emit(&[0x4C, 0x89, 0x45, 0x08]);
    }

    if let Some(keys) = profile.special_rule_keys {
        for (offset, key) in [0x18u8, 0x1A, 0x1C].into_iter().zip(keys) {
            builder.emit(&[0x66, 0xC7, 0x45, offset]);
            builder.emit(&key.to_le_bytes());
        }
    }

    if let Some(terrain) = profile.terrain_value {
        builder.emit(&[0xC6, 0x45, 0x1F, terrain]);
    }

    if let Some(counter) = counter_address {
        builder.emit(&[0x49, 0xBB]);
        builder.emit(&counter.to_le_bytes());
        builder.emit(&[0xF0, 0x49, 0xFF, 0x03]);
    }

    builder.mark("done")?;
    builder.emit(&[
        0x41, 0x5B, 0x41, 0x5A, 0x41, 0x59, 0x41, 0x58, 0x5A, 0x59, 0x58, 0x9D,
    ]);
    builder.emit(original_instruction);
    builder.emit(&[0x48, 0xB8]);
    builder.emit(&return_address.to_le_bytes());
    builder.emit(&[0xFF, 0xE0]);
    builder.finish()
}

/// `build_challenge_trampoline`.
///
/// The verified getter passes the seed in `EDX` and returns the capacity in
/// `AL/EAX`, so a matching seed returns immediately while every other seed runs
/// the original instruction and returns to the hook site.
pub fn build_challenge_trampoline(
    seed: u32,
    capacity: u8,
    return_address: u64,
    counter_address: u64,
    original_instruction: &[u8],
) -> Result<Vec<u8>, RuntimeError> {
    if !(1..=7).contains(&capacity) {
        return Err(RuntimeError::InvalidOverrideProfile {
            detail: "capacity must be from 1 to 7".to_string(),
        });
    }
    if original_instruction.is_empty() {
        return Err(RuntimeError::InvalidOverrideProfile {
            detail: "original_instruction must not be empty".to_string(),
        });
    }
    let mut code = Vec::with_capacity(64);
    code.extend_from_slice(&[0x9C, 0x81, 0xFA]);
    code.extend_from_slice(&seed.to_le_bytes());
    let mut matched = Vec::with_capacity(24);
    matched.push(0x50);
    matched.extend_from_slice(&[0x48, 0xB8]);
    matched.extend_from_slice(&counter_address.to_le_bytes());
    matched.extend_from_slice(&[0xF0, 0x48, 0xFF, 0x00]);
    matched.push(0x58);
    matched.push(0x9D);
    matched.push(0xB8);
    matched.extend_from_slice(&(capacity as u32).to_le_bytes());
    matched.push(0xC3);
    code.push(0x75); // jnz normal
    code.push(matched.len() as u8);
    code.extend_from_slice(&matched);
    code.push(0x9D);
    code.extend_from_slice(original_instruction);
    code.extend_from_slice(&[0xFF, 0x25, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&return_address.to_le_bytes());
    Ok(code)
}

struct CodeBuilder {
    code: Vec<u8>,
    labels: Vec<(String, usize)>,
    fixups: Vec<(usize, String)>,
}

impl CodeBuilder {
    fn new() -> Self {
        Self {
            code: Vec::new(),
            labels: Vec::new(),
            fixups: Vec::new(),
        }
    }

    fn emit(&mut self, data: &[u8]) {
        self.code.extend_from_slice(data);
    }

    fn branch(&mut self, opcode: &[u8], label: &str) {
        self.emit(opcode);
        let offset = self.code.len();
        self.emit(&[0, 0, 0, 0]);
        self.fixups.push((offset, label.to_string()));
    }

    fn mark(&mut self, label: &str) -> Result<(), RuntimeError> {
        if self.labels.iter().any(|(name, _)| name == label) {
            return Err(RuntimeError::InvalidOverrideProfile {
                detail: format!("duplicate code label: {label}"),
            });
        }
        self.labels.push((label.to_string(), self.code.len()));
        Ok(())
    }

    fn finish(self) -> Result<Vec<u8>, RuntimeError> {
        let mut code = self.code;
        for (offset, label) in self.fixups {
            let target = self
                .labels
                .iter()
                .find(|(name, _)| *name == label)
                .map(|(_, position)| *position)
                .ok_or_else(|| RuntimeError::InvalidOverrideProfile {
                    detail: format!("unresolved code label: {label}"),
                })?;
            let displacement = target as i64 - (offset as i64 + 4);
            let value =
                i32::try_from(displacement).map_err(|_| RuntimeError::InvalidOverrideProfile {
                    detail: format!("code label {label} is out of branch range"),
                })?;
            code[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        Ok(code)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_challenge_trampoline, build_override_trampoline, build_relative_jump, EnemyGroup,
        OverrideProfile, TRAMPOLINE_CAPACITY,
    };
    use crate::error::RuntimeError;

    fn bytes(code: &[u8], from: usize, to: usize) -> Vec<u8> {
        code[from..to].to_vec()
    }

    #[test]
    fn relative_jumps_are_five_bytes_and_range_checked() -> Result<(), RuntimeError> {
        assert_eq!(
            build_relative_jump(0x1000, 0x2000)?,
            vec![0xE9, 0xFB, 0x0F, 0x00, 0x00]
        );
        assert_eq!(
            build_relative_jump(0x1000, 0x1000)?,
            vec![0xE9, 0xFB, 0xFF, 0xFF, 0xFF]
        );
        assert!(matches!(
            build_relative_jump(0x1000, 0x8000_0000_0000),
            Err(RuntimeError::TrampolineOutOfRange { .. })
        ));
        Ok(())
    }

    #[test]
    fn an_empty_profile_is_refused() {
        let profile = OverrideProfile {
            seed: 1,
            enemy_groups: Vec::new(),
            special_rule_keys: None,
            terrain_value: None,
        };
        assert!(matches!(
            build_override_trampoline(&profile, 0x1000, None, &[0x90]),
            Err(RuntimeError::InvalidOverrideProfile { .. })
        ));
    }

    #[test]
    fn a_trampoline_ends_with_the_original_bytes_and_a_rip_indirect_jump(
    ) -> Result<(), RuntimeError> {
        let profile = OverrideProfile {
            seed: 0x11223344,
            enemy_groups: Vec::new(),
            special_rule_keys: Some([0x11, 0x22, 0x33]),
            terrain_value: Some(0x07),
        };
        let code = build_override_trampoline(
            &profile,
            0x1_4000_0000,
            Some(0x2_0000_0000),
            &[0x48, 0x8B, 0x54, 0x24, 0x60],
        )?;
        assert!(code.len() < TRAMPOLINE_CAPACITY);
        let length = code.len();
        // Tail: `movabs rax, imm64` then `jmp rax`.
        assert_eq!(bytes(&code, length - 12, length - 10), vec![0x48, 0xB8]);
        assert_eq!(
            bytes(&code, length - 10, length - 2),
            0x1_4000_0000u64.to_le_bytes()
        );
        assert_eq!(bytes(&code, length - 2, length), vec![0xFF, 0xE0]);
        assert_eq!(
            bytes(&code, length - 17, length - 12),
            vec![0x48, 0x8B, 0x54, 0x24, 0x60]
        );
        assert!(
            code.windows(10).any(|window| {
                &window[..2] == [0x49, 0xBB].as_slice()
                    && window[2..] == 0x2_0000_0000u64.to_le_bytes()
            }),
            "the counter store carries the counter address"
        );
        Ok(())
    }

    #[test]
    fn the_challenge_trampoline_returns_the_capacity_only_for_its_seed() -> Result<(), RuntimeError>
    {
        let code = build_challenge_trampoline(
            0xAABBCCDD,
            5,
            0x1_4000_1000,
            0x2_0000_0000,
            &[0x48, 0x89, 0x5C, 0x24, 0x08],
        )?;
        assert_eq!(
            bytes(&code, 0, 7),
            vec![0x9C, 0x81, 0xFA, 0xDD, 0xCC, 0xBB, 0xAA]
        );
        assert_eq!(code[7], 0x75);
        let matched = code[8] as usize;
        assert_eq!(code[9 + matched], 0x9D);
        assert_eq!(
            bytes(&code, 10 + matched, 15 + matched),
            vec![0x48, 0x89, 0x5C, 0x24, 0x08]
        );
        let length = code.len();
        assert_eq!(
            bytes(&code, length - 8, length),
            0x1_4000_1000u64.to_le_bytes()
        );
        Ok(())
    }

    #[test]
    fn enemy_groups_require_a_role_with_every_key() -> Result<(), RuntimeError> {
        let profile = OverrideProfile {
            seed: 7,
            enemy_groups: vec![EnemyGroup {
                lookup_key: 0x0000_1234,
                role: 3,
            }],
            special_rule_keys: None,
            terrain_value: None,
        };
        let code = build_override_trampoline(
            &profile,
            0x1000_0000,
            Some(0x2000_0000),
            &[0x48, 0x8B, 0x54, 0x24, 0x60],
        )?;
        assert!(code
            .windows(8)
            .any(|window| window == [0x41, 0xC7, 0x40, 0x04, 0x34, 0x12, 0x00, 0x00].as_slice()));
        assert!(code
            .windows(8)
            .any(|window| window == [0x41, 0xC7, 0x40, 0x08, 0x03, 0x00, 0x00, 0x00].as_slice()));
        Ok(())
    }
}
