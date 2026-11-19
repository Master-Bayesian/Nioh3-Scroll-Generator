//! Candidate-owned PC202 normal-owner SOURCE reconstruction.
//!
//! 376C6C -> 377440 compiles one prefix and 9ABC5C copies it, before
//! 377394 invokes core818B68 then selected818B68. Inherited masks read that
//! generation's core, never the captured completed aggregate. The input
//! program contains ordered producer submissions, not fitted group totals.
//! Guardian8508 is evaluated after equipment/sets and after each submission.
//! The three3F9FC0 groups are updated only after materialization.

use super::{i32_at, selected_map, u16_at, u32_at, Character, Entry, Frame, Scratch, Source, Tables, R, BANK, ITEM};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "nioh3-normal-source-rebuild/v1";
const LATE_GROUPS: [u16; 3] = [0x1ee2, 0xc437, 0xd12b];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Submission {
    pub group: u16,
    pub value: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GuardianSubmission {
    pub slot: u8,
    pub ordinal: u8,
    pub group: u16,
    pub value: u32,
    pub requirement: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CoreInput {
    /// Exact232-byte producer record; no missing-byte completion is allowed.
    pub record: Vec<u8>,
    /// Original554268 unlock decision, derived from raw progressAC/curve720.
    pub unlocked: bool,
    /// Original554268 passive118/record-flag mask, before557880 collection.
    pub effect_mask: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceScope {
    /// Executable, explicit conditional inputs. Never licenses app TYPE.
    OfflineConditional,
    /// Compiler/copy/core/selected captured in one process/thread generation.
    RebuildEpoch,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub scope: EvidenceScope,
    pub input_sha256: String,
    pub tables_sha256: String,
    pub generation: String,
    pub form: i32,
    pub native_item_category: u32,
    pub compiler_return_rva: u32,
    pub owner_kind: u32,
    pub item_sync: bool,
    pub second_weapon_pass: bool,
    /// The shared raw input digest excludes gear but includes every producer
    /// input. Refresh must replace or retire this program as one object.
    pub runtime_input_sha256: Option<String>,
    /// Reserved receipt identity. A string is not proof: RebuildEpoch remains
    /// refused until a complete runtime ingestion implementation is present.
    pub epoch_receipt_sha256: Option<String>,
    /// Explicit character inputs shared by the compiler and both rebuilds.
    pub character: CharacterBinding,
    pub source_scalar_words: [u32; 9],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterBinding {
    pub active: usize,
    pub cap: u32,
    pub plus_cap: u32,
    pub style: i32,
    pub unarmed: u32,
    pub level: u32,
}

/// Reference STAT must be the same actor/attributes as the program. Existing
/// installed contexts may intentionally retain an older canonical baseline.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineBinding {
    pub stat_sha256: String,
    pub reset_d_sha256: String,
    pub attrs: [i32; 7],
    pub health_seed: u32,
    pub defense_sources: u32,
    pub main_base: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Program {
    pub schema: String,
    pub binding: Binding,
    pub prefix: Vec<Submission>,
    pub active_guardian: u8,
    pub guardian_requirement: u16,
    pub guardians: Vec<GuardianSubmission>,
    pub cores: Vec<CoreInput>,
    /// Four raw clan records in original order (3FA060 returns first match).
    pub late_clan: [Submission; 4],
    /// Explicit projection of actual552F98 selectors to rebuilt inventory.
    /// When this has not been witnessed the program remains diagnostic.
    pub post_inventory_projection: bool,
    /// Fully known raw owner inventory from this input generation. Only the
    /// ten searched positions may vary. Reserve/equipment selectors stay bound.
    pub reference_inventory: Vec<u8>,
    /// Actual552F98 NULL/non-NULL decisions, bank then [13,9,10,15]. A new
    /// owned accessory creates a non-NULL selection even if the slot was empty.
    pub post_record_present: [[bool; 4]; 2],
    /// None permits source-oracle evaluation only. It cannot score an attack.
    pub baseline: Option<BaselineBinding>,
}

fn digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Program {
    pub fn load(raw: &[u8]) -> R<Self> {
        if raw.len() > 1024 * 1024 {
            return Err("source rebuild program exceeds its bounded input size");
        }
        let program: Self = serde_json::from_slice(raw).map_err(|_| "invalid source rebuild program")?;
        program.validate()?;
        Ok(program)
    }

    pub fn validate(&self) -> R<()> {
        let b = &self.binding;
        if self.schema != SCHEMA || !digest(&b.input_sha256) || !digest(&b.tables_sha256)
            || b.generation.is_empty() || b.generation.len() > 256
            || !matches!(b.form, 0 | 1) || b.compiler_return_rva != 0x3774ad
            || b.owner_kind != 0 || b.item_sync || b.second_weapon_pass
            || b.source_scalar_words.iter().enumerate().any(|(index, value)| index != 1 && *value != 0)
            || b.character.active > 1
        {
            return Err("source rebuild normal-owner contract is not supported");
        }
        if self.prefix.len() > 4096 || self.active_guardian > 1 || self.guardians.len() > 12
            || self.cores.len() != 8
            || self.reference_inventory.len() != 0x2018
            || self.cores.iter().any(|c| c.record.len() != ITEM || c.effect_mask & 0x80 != 0)
        {
            return Err("source rebuild producer input extent differs");
        }
        let mut previous = None;
        for event in &self.guardians {
            let key = (event.slot, event.ordinal);
            if event.slot > 1 || event.ordinal > 5 || previous.is_some_and(|p| p >= key) {
                return Err("guardian submissions are not in native slot/ordinal order");
            }
            previous = Some(key);
        }
        if self.baseline.as_ref().is_some_and(|baseline|
            !digest(&baseline.stat_sha256) || !digest(&baseline.reset_d_sha256)
                || baseline.attrs.iter().any(|a| !(1..=999).contains(a)))
        {
            return Err("source rebuild baseline binding is malformed");
        }
        if b.scope == EvidenceScope::RebuildEpoch {
            // A JSON label or a digest-shaped string is not a native lifetime
            // proof. Until fresh compiler inputs and a verified single-epoch
            // trace are ingested together, this transition remains closed.
            return Err("runtime source epoch ingestion is not implemented; conditional evidence cannot authorize TYPE");
        }
        Ok(())
    }

    pub(crate) fn check_candidate(&self, tables: &Tables, c: &Character) -> R<()> {
        if c.inv.len() != 0x2018 {
            return Err("source rebuild candidate inventory extent differs");
        }
        if self.binding.tables_sha256 != tables.source_digest {
            return Err("source rebuild belongs to different parameter tables");
        }
        if c.mode != self.binding.form || c.core_mode != c.mode || c.active > 1 {
            return Err("source rebuild candidate changed the compiled owner form");
        }
        let expected = &self.binding.character;
        if (c.active, c.cap, c.plus_cap, c.style, c.unarmed, c.level)
            != (expected.active, expected.cap, expected.plus_cap, expected.style, expected.unarmed, expected.level)
        {
            return Err("source rebuild character/cap inputs changed generation");
        }
        let bank_start = c.active * BANK;
        let retained = [0..bank_start, bank_start + 13 * ITEM..c.inv.len()];
        if retained.into_iter().any(|range| c.inv[range.clone()] != self.reference_inventory[range]) {
            return Err("source rebuild retained inventory changed generation");
        }
        let held = bank_start..bank_start + ITEM;
        let spares = bank_start + ITEM..bank_start + 4 * ITEM;
        if c.inv[spares.clone()] != self.reference_inventory[spares.clone()] {
            let mut moved = c.inv[spares.clone()].as_chunks::<ITEM>().0.iter().map(|row| row.as_slice())
                .zip(self.reference_inventory[spares].as_chunks::<ITEM>().0.iter().map(|row| row.as_slice()))
                .filter(|(now, before)| now != before);
            let valid = match (moved.next(), moved.next()) {
                (Some((now, before)), None) => before == &c.inv[held.clone()]
                    && now == &self.reference_inventory[held]
                    && c.inv[bank_start + 0xF68] != 0,
                _ => false,
            };
            if !valid {
                return Err("source rebuild retained inventory changed generation");
            }
        }
        // Original401B34/555B08 follows inventory selected bank and selector,
        // not a skill name or a UI weapon ID. All same-category bodies share
        // this compiled prefix; a different category requires recompilation.
        let bank = i32_at(&c.inv, 0x1ee0);
        if !matches!(bank, 0 | 1) { return Err("source rebuild inventory bank is invalid"); }
        let bank = bank as usize;
        if bank != c.active { return Err("source rebuild inventory and character active banks differ"); }
        let slot = usize::from(c.inv[bank * BANK + 0xf68] == 0);
        let key = u16_at(&c.inv, bank * BANK + slot * ITEM);
        let category = tables.items.row(u32::from(key)).map_or(0x3c2e, |row| u32_at(row, 0x58));
        if category != self.binding.native_item_category {
            return Err("candidate weapon category requires a new source compiler prefix");
        }
        Ok(())
    }

    pub(super) fn submit_prefix(&self, tables: &Tables, source: &mut Source, mode: i32) {
        for op in &self.prefix { tables.aggregate(source, mode, op.group, op.value); }
    }

    pub(super) fn after_sets(&self, tables: &Tables, c: &Character, s: &mut Scratch) -> R<()> {
        let bank = selected_map(c.mode, c.mode);
        for op in &self.guardians {
            if op.group == 0 || self.guardian_requirement < op.requirement { continue; }
            let other_enabled = s.source.maps[bank].iter().find(|e| e.key == 0x8508)
                .is_some_and(|e| e.base.wrapping_add(u32::from(e.dup)) as i32 >= 0);
            if op.slot != self.active_guardian && !other_enabled { continue; }
            tables.aggregate(&mut s.source, c.mode, op.group, op.value);
        }
        for core in &self.cores {
            if core.unlocked {
                // Native side entries are collected before the effect mask.
                // They may affect late3FAD08 but do not replay earlier sets.
                tables.collect(&mut s.source, &core.record, false, c.mode, core.effect_mask)?;
            }
        }
        Ok(())
    }

    fn inventory_max(&self, tables: &Tables, c: &Character, group: u16) -> u32 {
        let mut maximum = 0i32;
        for bank in 0..2 {
            let mut sum = 0u32;
            for (ordinal, slot) in [13, 9, 10, 15].into_iter().enumerate() {
                let at = bank * BANK + slot * ITEM;
                let item = &c.inv[at..][..ITEM];
                let changed = item != &self.reference_inventory[at..][..ITEM];
                if !self.post_record_present[bank][ordinal] && !changed { continue; }
                // Original1075ED0 reads raw affixes, without item qualification,
                // ordinary masks, side exclusions, or aggregate caps.
                for ordinal in 0..7 {
                    let effect = u32_at(item, 0x38 + ordinal * 24);
                    if effect != u32::MAX && tables.effects.row(effect)
                        .is_some_and(|row| u16_at(row, 2) == group)
                    {
                        sum = sum.wrapping_add(u32_at(item, 0x3c + ordinal * 24));
                    }
                }
            }
            maximum = maximum.max(sum as i32);
        }
        maximum as u32
    }

    pub(super) fn finish(&self, tables: &Tables, c: &Character, source: &mut Source) -> R<()> {
        if !self.post_inventory_projection {
            return Err("post-source inventory selectors have not been bound to the candidate");
        }
        let bank = selected_map(i32_at(&c.inv, 0x1ee0), c.mode);
        for group in LATE_GROUPS {
            let mut amount = self.inventory_max(tables, c, group);
            for side in &source.sides[bank] {
                let count = u32::from(u16_at(side, 4)).wrapping_add(u32_at(side, 8));
                if count <= 1 { continue; }
                let Some(parameter) = tables.sides.row(u32_at(side, 0)) else { continue; };
                for index in 0..7 {
                    let reward = &parameter[0x30 + index * 24..][..24];
                    let threshold = u32_at(reward, 0);
                    if u16_at(reward, 8) != group || count < threshold { continue; }
                    let coordinate = threshold.wrapping_sub(u32_at(side, 8)) as i32;
                    let offset = if coordinate > 0 { 0x1c + 2 * coordinate as i64 } else { 0x1e };
                    if !(0..=66).contains(&offset) { return Err("late set reward leaves its side record"); }
                    amount = amount.wrapping_add(tables.curve(reward, u16_at(side, offset as usize)));
                }
            }
            if tables.groups.row(u32::from(group)).is_some() {
                if let Some(clan) = self.late_clan.iter().find(|op| op.group == group) {
                    amount = amount.wrapping_add(clan.value);
                }
            }
            if amount as i32 > 0 { late_max(&mut source.maps[bank], group, amount, tables); }
        }
        Ok(())
    }

    pub(super) fn pipeline(&self, tables: &Tables, f: &Frame, c: &Character,
        attrs: &[i32; 7], ops: &[(u16, i64)], output: &mut Scratch) -> R<()>
    {
        self.check_candidate(tables, c)?;
        if !ops.is_empty() || !f.native_before.is_empty() || !f.native_after.is_empty() {
            return Err("normal source rebuild cannot mix fitted or flattened source layers");
        }
        let mut core = c.clone();
        let mut state = Scratch::new();
        state.source.mode = c.mode;
        state.source.thresholds = attrs.map(|a| a as u32);
        self.submit_prefix(tables, &mut state.source, c.mode);
        let prefix = state.source.maps.clone();
        core.core_maps = prefix.clone();
        tables.collect_pipeline(f, &core, attrs, &[], output, Some(self))?;

        // Equipment collection, sets, guardian gates and late SOURCE groups
        // depend on the same inventory and inputs in both passes. Inherited
        // core maps enter only prepared_attack_pair. Keep the pre-tail map
        // for materialization and use the completed map for the weapon pair.
        let bank = selected_map(core.mode, core.mode);
        let pre_tail = output.source.maps[bank].clone();
        self.finish(tables, &core, &mut output.source)?;
        core.core_maps = output.source.maps.clone();
        tables.refresh_prepared_attacks(&core, attrs, output)?;
        tables.materialize_entries(&core, attrs, &pre_tail, &mut output.stat, &mut output.d)?;
        output.source_rebuild_trace = Some(Trace { prefix, completed_core: core.core_maps });
        Ok(())
    }
}

/// Original1AB5300: distinct from557BA0. An update compares against base+dup,
/// writes only base, and retains dup. New entries bypass group qualification.
fn late_max(entries: &mut Vec<Entry>, group: u16, value: u32, tables: &Tables) {
    // Original1AB5300 returns before any source access for the zero group.
    if group == 0 { return; }
    if let Some(entry) = entries.iter_mut().find(|entry| entry.key == group
        && tables.groups.row(u32::from(group)).is_some())
    {
        if value > entry.base.wrapping_add(u32::from(entry.dup)) { entry.base = value; }
    } else if entries.len() < 256 {
        entries.push(Entry { key: group, base: value, dup: 0 });
    }
}

#[derive(Clone, Debug)]
pub struct Trace {
    pub prefix: [Vec<Entry>; 2],
    pub completed_core: [Vec<Entry>; 2],
}

#[cfg(test)]
#[path = "late_source_native_control.rs"]
mod late_native_control;
