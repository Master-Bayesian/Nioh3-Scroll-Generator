//! PC202 static damage descriptors. A row selector is local to one exact
//! resource/action/parameter identity and does not imply a landed contact.
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[path = "action_controller.rs"]
mod action_controller;
pub use action_controller::{ActionController, CollisionComponent, CollisionRoute};

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

fn parameter_digest(hex: &str) -> Result<[u8; 32], &'static str> {
    if hex.len() != 64 || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("invalid attack parameter digest");
    }
    let mut bytes = [0u8; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16)
            .map_err(|_| "invalid attack parameter digest")?;
    }
    Ok(bytes)
}

impl ActionRow {
    pub fn resolve(&self) -> Result<ResolvedActionRow, &'static str> {
        let parameter = parameter_digest(&self.parameter_sha256)?;
        let key = Sha256::new()
            .chain_update(self.resource.as_bytes())
            .chain_update([0])
            .chain_update(self.action.to_le_bytes())
            .chain_update(self.row.to_le_bytes())
            .chain_update(parameter)
            .finalize()
            .into();
        let row = ResolvedActionRow::lookup(key)?;
        if format!("{:x}", Sha256::digest(row.bytes)) != self.row_sha256 {
            return Err("selected damage row digest differs from its descriptor");
        }
        Ok(row)
    }
}

impl ResolvedActionRow {
    fn lookup(key: [u8; 32]) -> Result<Self, &'static str> {
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
        let (entries, _) = index.as_chunks::<36>();
        let position = entries
            .binary_search_by(|entry| entry[..32].cmp(&key))
            .map_err(|_| "unbound resource/action/parameter/row combination")?;
        let slot = word(&entries[position][32..]).ok_or_else(invalid)? as usize;
        let row = rows.get(slot).ok_or_else(invalid)?;
        Ok(ResolvedActionRow { key, bytes: row })
    }
}

pub struct ResolvedActionRow {
    pub key: [u8; 32],
    pub bytes: &'static [u8; ROW_BYTES],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduledProjectile {
    pub resource: String,
    pub action: u16,
    pub parameter_sha256: String,
    pub projectile_slot: u16,
}

pub struct ProjectileRow {
    pub damage: ResolvedActionRow,
    pub carrier: u32,
    pub target_action: u16,
    pub selector: u8,
    pub global_key: i32,
    pub imbue: ProjectileImbue,
    pub descriptor_fields: [u32; 3],
}

/// REA 65E908 copies spawn+C0/C4 into object+5F8/5FC. This selects
/// an imbue lookup branch; both branches can use candidate weapon attack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectileImbue {
    pub selector: i32,
    pub flags: u32,
}

impl ProjectileImbue {
    pub fn checks_owner(self) -> bool {
        self.selector != -1 || self.flags & 0x40000 != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimaryDamage {
    Action { key: [u8; 32] },
    Global { index: u16 },
    NoPrimary,
}

/// A file-declared occurrence. Timing and damage-row selection do not establish a landed hit.
pub struct SpawnReference {
    pub slot: u16,
    pub tick: u16,
    pub event_key: u16,
    pub carrier: u32,
    pub requested_action: u16,
    pub requested_row: i8,
    pub global_key: i32,
    pub primary: PrimaryDamage,
    pub imbue: ProjectileImbue,
    pub descriptor_fields: [u32; 3],
}

struct SpawnBank {
    occurrences: &'static [[u8; 99]],
    globals: &'static [[u8; ROW_BYTES]],
}

impl SpawnBank {
    fn load() -> Result<Self, &'static str> {
        let invalid = "invalid PC202 projectile row index";
        let (header, body) = include_bytes!("pc202_projectile_rows.bin")
            .split_first_chunk::<16>()
            .ok_or(invalid)?;
        let size = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        let count = u32::from_le_bytes(header[12..16].try_into().unwrap()) as usize;
        let (events, table) = body
            .split_at_checked(size.checked_mul(99).ok_or(invalid)?)
            .ok_or(invalid)?;
        let (globals, rest) = table.as_chunks();
        if &header[..8] != b"N3PRJ003" || !rest.is_empty() || globals.len() != count {
            return Err(invalid);
        }
        Ok(Self {
            occurrences: events.as_chunks().0,
            globals,
        })
    }
}

pub fn scheduled_projectiles(
    resource: &str,
    action: u16,
    digest: &str,
) -> Result<Vec<SpawnReference>, &'static str> {
    let bank = SpawnBank::load()?;
    let key = Sha256::new()
        .chain_update(resource.as_bytes())
        .chain_update([0])
        .chain_update(action.to_le_bytes())
        .chain_update(parameter_digest(digest)?)
        .finalize();
    let start = bank.occurrences.partition_point(|row| row[..32] < key[..]);
    Ok(bank.occurrences[start..]
        .iter()
        .take_while(|row| row[..32] == key[..])
        .map(|row| {
            let short = |at: usize| u16::from_le_bytes([row[at], row[at + 1]]);
            let requested_action = short(42);
            let requested_row = row[44] as i8;
            let index = short(45);
            let primary = if requested_action != u16::MAX && requested_row >= 0 {
                PrimaryDamage::Action {
                    key: row[47..79].try_into().unwrap(),
                }
            } else if usize::from(index) < bank.globals.len() {
                PrimaryDamage::Global { index }
            } else {
                PrimaryDamage::NoPrimary
            };
            SpawnReference {
                slot: short(32),
                tick: short(34),
                event_key: short(36),
                carrier: u32::from_le_bytes(row[38..42].try_into().unwrap()),
                requested_action,
                requested_row,
                global_key: if index == u16::MAX {
                    -1
                } else {
                    i32::from(index)
                },
                primary,
                imbue: ProjectileImbue {
                    selector: i32::from_le_bytes(row[79..83].try_into().unwrap()),
                    flags: u32::from_le_bytes(row[83..87].try_into().unwrap()),
                },
                descriptor_fields: std::array::from_fn(|i| {
                    u32::from_le_bytes(row[87 + i * 4..91 + i * 4].try_into().unwrap())
                }),
            }
        })
        .collect())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialProjectile {
    pub object: u64,
    pub descriptor_head: [u32; 4],
    pub descriptor_5f8: i32,
    pub descriptor_5fc: u32,
    pub packet_flags: u32,
    pub secondary_element_selector: bool,
}

impl InitialProjectile {
    pub fn matches(&self, object: u64, descriptor_kind: u8, source_tag: u32) -> bool {
        self.object != 0
            && self.object.checked_add(0x864).is_some()
            && object == self.object
            && descriptor_kind == 0
            && self.descriptor_head[1] == 0
            && source_tag == self.descriptor_head[0].wrapping_shl(4) | 4
    }
}

impl ScheduledProjectile {
    pub fn resolve(&self) -> Result<ProjectileRow, &'static str> {
        let reference = scheduled_projectiles(&self.resource, self.action, &self.parameter_sha256)?
            .into_iter()
            .find(|r| r.slot == self.projectile_slot)
            .ok_or("no scheduled projectile at the selected slot")?;
        let damage =
            match reference.primary {
                PrimaryDamage::Action { key } => ResolvedActionRow::lookup(key)?,
                PrimaryDamage::Global { index } => {
                    let bytes = &SpawnBank::load()?.globals[usize::from(index)];
                    let key = Sha256::new()
                        .chain_update(b"PC202-global-damage\0")
                        .chain_update(index.to_le_bytes())
                        .chain_update(bytes)
                        .finalize()
                        .into();
                    ResolvedActionRow { key, bytes }
                }
                PrimaryDamage::NoPrimary => return Err(
                    "spawn has no primary damage row; attached events require their own binding",
                ),
            };
        Ok(ProjectileRow {
            damage,
            carrier: reference.carrier,
            target_action: reference.requested_action,
            selector: reference.requested_row as u8,
            global_key: reference.global_key,
            imbue: reference.imbue,
            descriptor_fields: reference.descriptor_fields,
        })
    }
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
    fn one_carrier_uses_different_imbue_branches_across_weapon_resources(
    ) -> Result<(), &'static str> {
        for (resource, action, param, slot, expected) in [
            (
                "0x600000131",
                25523,
                "95a4eb857eeb07bd26ad225f8737d263cbd9ba9731a1a0b365e47b92d28c69bd",
                16,
                (-1, 0x40100, true),
            ),
            (
                "0x6000000fd",
                25166,
                "d0b2e2b9f56f017704db6fb58e300cd8e6f38f47b5ebb3681870bfe828711b4b",
                0,
                (1, 0x100, true),
            ),
            (
                "0x600000184",
                25281,
                "85990b1f1b552c199c9c7a9c79ecb303554acb1bad677b73906606624432d91e",
                7,
                (-1, 0x100, false),
            ),
        ] {
            let events = scheduled_projectiles(resource, action, param)?;
            let r = events
                .iter()
                .find(|r| r.slot == slot)
                .ok_or("missing occurrence")?;
            assert_eq!((r.carrier, r.descriptor_fields[0]), (783025, 0));
            assert_eq!(
                (r.imbue.selector, r.imbue.flags, r.imbue.checks_owner()),
                expected
            );
        }
        Ok(())
    }

    #[test]
    fn global_projectile_row_and_no_primary_are_distinct_across_resources(
    ) -> Result<(), &'static str> {
        for (resource, action, param) in [
            (
                "0x60000000d",
                1725,
                "58c50b3be6e3669133d631f8d586232d7ad7c20c1cbb41a3fba4c47c6fa697db",
            ),
            (
                "0x6000000fd",
                25101,
                "09653eb4c71e7f504e5dbd7c2d06201f545ece2e979b5a95d6fec17fb05fc104",
            ),
            (
                "0x600000184",
                1725,
                "7bbe2e983535b873b170bc5a40bdeddfb13a364b7ebc039029d73e5323d2e827",
            ),
        ] {
            let refs = scheduled_projectiles(resource, action, param)?;
            let r = refs.iter().find(|r| r.slot == 2).unwrap();
            assert_eq!(
                (r.tick, r.event_key, r.primary),
                (0, 20000, PrimaryDamage::Global { index: 372 })
            );
            let occurrence = ScheduledProjectile {
                resource: resource.into(),
                action,
                parameter_sha256: param.into(),
                projectile_slot: 2,
            };
            let projected = occurrence.resolve()?;
            assert_eq!(projected.global_key, 372);
            assert_eq!(
                format!("{:x}", Sha256::digest(projected.damage.bytes)),
                "5c71e4f7237d9b15c2eddc977769edce7ea9b56a25076f97572509d66026f0ed"
            );
        }
        let param = "9ce5b656d42473316efeb7f56e4b91e434dd8de20316f2a4fb9886203d75f9ee";
        let refs = scheduled_projectiles("0x60000000d", 25027, param)?;
        assert_eq!(
            refs.iter().find(|r| r.slot == 1).unwrap().primary,
            PrimaryDamage::NoPrimary
        );
        let absent = ScheduledProjectile {
            resource: "0x60000000d".into(),
            action: 25027,
            parameter_sha256: param.into(),
            projectile_slot: 1,
        };
        assert!(absent.resolve().is_err());
        Ok(())
    }

    #[test]
    fn flying_circle_empty_owner_selects_charge_specific_projectile() -> Result<(), &'static str> {
        for (action, digest, selector, expected) in [
            (
                25203,
                "35b1330ed3b3621825d31577d9ef0dd8e83b2decdf78b479ad6ee4a1304c8fa8",
                0,
                "ebc77d31b61a3c719de5ae8a5d196b54698924491eb84f71f975e38b239e4434",
            ),
            (
                25204,
                "ab1c73caa414ae60b33a1e5a43b0acd96f8e86437d0e6383261df741021186fa",
                1,
                "9d642b2b610e21c932b7364f5a422113975e438866f258ca00413067b70107d3",
            ),
        ] {
            let owner = ActionController::resolve("0x6000000fd", action, digest)?;
            assert_eq!(owner.first_index, 293);
            assert!(owner.rows.is_empty());
            let mut event = ScheduledProjectile {
                resource: "0x6000000fd".into(),
                action,
                parameter_sha256: digest.into(),
                projectile_slot: 0,
            };
            let row = event.resolve()?;
            assert_eq!(
                (row.carrier, row.target_action, row.selector),
                (783025, 25244, selector)
            );
            assert_eq!(format!("{:x}", Sha256::digest(row.damage.bytes)), expected);
            event.projectile_slot = 1;
            assert!(event.resolve().is_err());
        }
        Ok(())
    }

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
